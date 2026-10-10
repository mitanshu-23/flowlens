//! The project's vocabulary: every type, constant and static it defines,
//! with its documentation, its parts, and the functions that use it.
//!
//! Steps say "Count a new job"; a reader then wants to know what a `Job` is,
//! which fields it has, what `MAX_RETRIES` is set to. All of it is declared
//! in the code, usually with a doc comment, so it is read the same way the
//! steps are — from the syntax, nothing guessed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, ast, ast::AstToken, ast::HasName};

use super::Status;
use crate::model::*;
use crate::workspace::Workspace;

/// Longest definition kept for the "show definition" view, in lines.
const MAX_SOURCE_LINES: usize = 60;

/// One part of an entry: a struct field, an enum variant, a trait method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    /// The field's type, a variant's fields, a method's signature.
    pub ty: String,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub key: String,
    pub name: String,
    /// `struct`, `enum`, `trait`, `type`, `const`, `static`.
    pub kind: &'static str,
    pub doc: Option<String>,
    pub file: String,
    pub line: usize,
    pub parts: Vec<Part>,
    /// A constant's or static's type and value; a type alias's target.
    pub ty: Option<String>,
    pub value: Option<String>,
    /// The definition as written, comments and all.
    pub source: String,
    /// Functions that mention it, by their key.
    pub used_by: Vec<String>,
    pub status: Status,
    /// What it was before, when it changed.
    pub source_before: Option<String>,
}

/// Every entry of one workspace, keyed like functions are — `prefix` in place
/// of `crate` — plus, per function key, the entries it uses.
pub fn collect(ws: &Workspace, prefix: &str, base: &Path, fn_keys: &HashMap<GlobalId, String>) -> (BTreeMap<String, Entry>, HashMap<String, Vec<String>>) {
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut by_symbol: HashMap<GlobalId, String> = HashMap::new();
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();

    for (fid, f) in ws.files.iter().enumerate() {
        let root = SourceFile::parse(&f.src, Edition::Edition2021).syntax_node();
        let abs = Path::new(&f.path).canonicalize().unwrap_or_else(|_| f.path.clone().into());
        let file = abs.strip_prefix(&base).unwrap_or(Path::new(&f.path)).to_string_lossy().into_owned();
        let module = |path: &[String]| {
            let mut parts = vec![prefix.to_string()];
            parts.extend(f.module_path.iter().cloned());
            parts.extend(path.iter().cloned());
            parts.join("::")
        };

        // By exact extent: an item shares its start with its first attribute,
        // and with the whole file when it comes first in it.
        let nodes: HashMap<(usize, usize), SyntaxNode> = root.descendants().map(|n| ((usize::from(n.text_range().start()), usize::from(n.text_range().end())), n)).collect();
        for s in &f.symbols {
            if !matches!(s.kind, SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait | SymbolKind::TypeAlias | SymbolKind::Const | SymbolKind::Static) || s.owner_kind.is_some() {
                continue;
            }
            let Some(node) = nodes.get(&s.range) else { continue };
            let key = format!("{}::{}", module(&s.module_path), s.name);
            let mut e = Entry {
                key: key.clone(),
                name: s.name.clone(),
                kind: s.kind.tag(),
                doc: s.doc.clone(),
                file: file.clone(),
                line: s.loc.line,
                parts: Vec::new(),
                ty: None,
                value: None,
                source: source_of(&f.src, s.range),
                used_by: Vec::new(),
                status: Status::Same,
                source_before: None,
            };
            describe(node, &mut e);
            by_symbol.insert(GlobalId { file: fid, sym: s.id }, key.clone());
            by_name.entry(s.name.clone()).or_default().push(key.clone());
            entries.insert(key, e);
        }

        // `lazy_static! { static ref POOL: PgPool = … }` declares statics
        // inside a macro the parser does not expand: read them from its text.
        for mac in root.descendants().filter_map(ast::MacroCall::cast) {
            let name = mac.path().map(|p| p.syntax().text().to_string()).unwrap_or_default();
            if !name.ends_with("lazy_static") {
                continue;
            }
            let Some(tt) = mac.token_tree() else { continue };
            let text = tt.syntax().text().to_string();
            let start: usize = mac.syntax().text_range().start().into();
            for decl in lazy_statics(&text) {
                let line = f.lines.line_col(&f.src, start).0 + text[..decl.at].matches('\n').count();
                let path: Vec<String> = mac.syntax().ancestors().filter_map(ast::Module::cast).filter_map(|m| m.name().map(|n| n.text().to_string())).collect::<Vec<_>>().into_iter().rev().collect();
                let key = format!("{}::{}", module(&path), decl.name);
                by_name.entry(decl.name.clone()).or_default().push(key.clone());
                entries.insert(
                    key.clone(),
                    Entry {
                        key,
                        name: decl.name,
                        kind: "static",
                        doc: decl.doc,
                        file: file.clone(),
                        line,
                        parts: Vec::new(),
                        ty: Some(decl.ty),
                        value: Some(decl.value),
                        source: decl.source,
                        used_by: Vec::new(),
                        status: Status::Same,
                        source_before: None,
                    },
                );
            }
        }
    }

    // Who uses what: references from inside a function that land on an
    // entry, by resolution where the indexer resolved them, by name for the
    // statics it cannot see inside their macro.
    let mut uses: HashMap<String, Vec<String>> = HashMap::new();
    for (fid, f) in ws.files.iter().enumerate() {
        for r in &f.references {
            let Some(from) = r.from.and_then(|sym| fn_keys.get(&GlobalId { file: fid, sym })) else {
                continue;
            };
            let target = match r.global_target(fid).and_then(|g| by_symbol.get(&g)) {
                Some(k) => Some(k.clone()),
                None if matches!(r.kind, RefKind::ValueRef | RefKind::TypeRef) => by_name.get(&r.name).filter(|ks| ks.len() == 1 && entries.get(&ks[0]).is_some_and(|e| e.kind == "static")).map(|ks| ks[0].clone()),
                None => None,
            };
            if let Some(t) = target {
                let list = uses.entry(from.clone()).or_default();
                if !list.contains(&t) {
                    list.push(t);
                }
            }
        }
    }
    for (f, ts) in &uses {
        for t in ts {
            if let Some(e) = entries.get_mut(t) {
                if !e.used_by.contains(f) {
                    e.used_by.push(f.clone());
                }
            }
        }
    }
    for e in entries.values_mut() {
        e.used_by.sort();
    }
    (entries, uses)
}

/// Mark what changed between two versions' entries, and merge them: removed
/// entries are kept so the reader sees they went.
pub fn compare(before: BTreeMap<String, Entry>, mut now: BTreeMap<String, Entry>) -> BTreeMap<String, Entry> {
    let same = |a: &str, b: &str| fingerprint(a) == fingerprint(b);
    let now_keys: HashSet<String> = now.keys().cloned().collect();
    for (k, e) in now.iter_mut() {
        match before.get(k) {
            None => e.status = Status::Added,
            Some(b) if !same(&b.source, &e.source) => {
                e.status = Status::Changed;
                e.source_before = Some(b.source.clone());
            }
            Some(_) => {}
        }
    }
    for (k, mut e) in before {
        if !now_keys.contains(&k) {
            e.status = Status::Removed;
            now.insert(k, e);
        }
    }
    now
}

/// Comments and whitespace do not change what a definition is.
fn fingerprint(source: &str) -> String {
    let parsed = SourceFile::parse(source, Edition::Edition2021);
    parsed.syntax_node().descendants_with_tokens().filter_map(|e| e.into_token()).filter(|t| t.kind() != SyntaxKind::WHITESPACE && t.kind() != SyntaxKind::COMMENT).map(|t| t.text().to_string()).collect::<Vec<_>>().join(" ")
}

/// Fill in an entry's parts from its definition.
fn describe(node: &SyntaxNode, e: &mut Entry) {
    if let Some(s) = ast::Struct::cast(node.clone()) {
        e.parts = fields(s.field_list());
    } else if let Some(en) = ast::Enum::cast(node.clone()) {
        for v in en.variant_list().into_iter().flat_map(|l| l.variants()) {
            e.parts.push(Part { name: v.name().map(|n| n.text().to_string()).unwrap_or_default(), ty: v.field_list().map(|f| one_line(f.syntax())).unwrap_or_default(), doc: doc_of(v.syntax()) });
        }
    } else if let Some(t) = ast::Trait::cast(node.clone()) {
        for item in t.assoc_item_list().into_iter().flat_map(|l| l.assoc_items()) {
            let syn = item.syntax();
            let name = ast::Fn::cast(syn.clone())
                .and_then(|f| f.name())
                .or_else(|| ast::TypeAlias::cast(syn.clone()).and_then(|t| t.name()))
                .or_else(|| ast::Const::cast(syn.clone()).and_then(|c| c.name()))
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            let head = one_line(syn);
            let head = head.split(['{', ';']).next().unwrap_or("").trim().to_string();
            e.parts.push(Part { name, ty: head, doc: doc_of(syn) });
        }
    } else if let Some(t) = ast::TypeAlias::cast(node.clone()) {
        e.ty = t.ty().map(|t| one_line(t.syntax()));
    } else if let Some(c) = ast::Const::cast(node.clone()) {
        e.ty = c.ty().map(|t| one_line(t.syntax()));
        e.value = value_of(node);
    } else if let Some(s) = ast::Static::cast(node.clone()) {
        e.ty = s.ty().map(|t| one_line(t.syntax()));
        e.value = value_of(node);
    }
}

fn fields(list: Option<ast::FieldList>) -> Vec<Part> {
    match list {
        Some(ast::FieldList::RecordFieldList(l)) => l.fields().map(|f| Part { name: f.name().map(|n| n.text().to_string()).unwrap_or_default(), ty: f.ty().map(|t| one_line(t.syntax())).unwrap_or_default(), doc: doc_of(f.syntax()) }).collect(),
        Some(ast::FieldList::TupleFieldList(l)) => l.fields().enumerate().map(|(i, f)| Part { name: i.to_string(), ty: f.ty().map(|t| one_line(t.syntax())).unwrap_or_default(), doc: doc_of(f.syntax()) }).collect(),
        None => Vec::new(),
    }
}

/// `= value;` of a constant or static, on one line.
fn value_of(node: &SyntaxNode) -> Option<String> {
    let text = one_line(node);
    let (_, v) = text.split_once('=')?;
    Some(clip(v.trim().trim_end_matches(';').trim(), 160))
}

/// `///` comments on a field, variant or item.
fn doc_of(node: &SyntaxNode) -> Option<String> {
    let mut lines = Vec::new();
    for child in node.children_with_tokens() {
        let Some(token) = child.as_token() else { break };
        if token.kind() != SyntaxKind::COMMENT {
            continue;
        }
        if let Some(c) = ast::Comment::cast(token.clone()) {
            if let Some(text) = c.doc_comment() {
                lines.push(text.trim().to_string());
            }
        }
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn source_of(src: &str, range: (usize, usize)) -> String {
    let text = &src[range.0..range.1.min(src.len())];
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= MAX_SOURCE_LINES {
        return text.to_string();
    }
    let mut out = lines[..MAX_SOURCE_LINES].join("\n");
    out.push_str(&format!("\n// … {} more lines", lines.len() - MAX_SOURCE_LINES));
    out
}

fn one_line(node: &SyntaxNode) -> String {
    // Leading doc comments are not part of the type or signature.
    let text: String = node.children_with_tokens().skip_while(|c| c.as_token().is_some_and(|t| t.kind() == SyntaxKind::COMMENT || t.kind() == SyntaxKind::WHITESPACE)).map(|c| c.to_string()).collect();
    clip(&text.split_whitespace().collect::<Vec<_>>().join(" "), 200)
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max - 1).collect();
    format!("{kept}\u{2026}")
}

/// A `static ref` inside `lazy_static! { … }`.
struct LazyStatic {
    name: String,
    ty: String,
    value: String,
    doc: Option<String>,
    source: String,
    /// Byte offset of the declaration within the macro's text.
    at: usize,
}

fn lazy_statics(text: &str) -> Vec<LazyStatic> {
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(i) = text[search..].find("static ref ") {
        let at = search + i;
        let rest = &text[at + "static ref ".len()..];
        let Some(colon) = rest.find(':') else { break };
        let name = rest[..colon].trim().to_string();
        let after = &rest[colon + 1..];
        let Some(eq) = after.find('=') else { break };
        let ty = after[..eq].split_whitespace().collect::<Vec<_>>().join(" ");
        // The value runs to the `;` that closes it, outside any brackets.
        let body = &after[eq + 1..];
        let (mut depth, mut end) = (0i32, body.len());
        for (j, ch) in body.char_indices() {
            match ch {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ';' if depth <= 0 => {
                    end = j;
                    break;
                }
                _ => {}
            }
        }
        let value = clip(&body[..end].split_whitespace().collect::<Vec<_>>().join(" "), 160);
        // `///` lines just above it.
        let before = &text[..at];
        let doc: Vec<&str> = before.lines().rev().skip(usize::from(!before.ends_with('\n'))).map(str::trim).take_while(|l| l.starts_with("///")).map(|l| l.trim_start_matches('/').trim()).collect();
        let doc = (!doc.is_empty()).then(|| doc.into_iter().rev().collect::<Vec<_>>().join("\n"));
        let consumed = at + "static ref ".len() + colon + 1 + eq + 1 + end;
        out.push(LazyStatic { source: format!("static ref {name}: {ty} = {};", body[..end].trim()), name, ty, value, doc, at });
        search = consumed.min(text.len());
    }
    out
}
