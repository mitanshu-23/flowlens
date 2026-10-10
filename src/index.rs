//! Builds a `FileIndex`: every definition in a file, every reference, and the
//! edges between them.
//!
//! Resolution is name-based, not type-based. There is no trait solver here, so
//! a bare `.foo()` is matched by method name; when several methods share that
//! name the reference is reported as `Ambiguous` rather than silently guessed.
//! Anything not defined in the file becomes `Unresolved`, tagged with the `use`
//! it most likely came from and classified as first- or third-party. Those tags
//! are what [`crate::workspace`] then follows to close cross-file edges, and
//! what keeps dependency calls out of the user's own call graph.
//!
//! Macro arguments are re-parsed rather than skipped: `format!("{}", esc(x))`
//! hides a real call to `esc` inside an unparsed token tree, and ignoring those
//! makes ordinary functions look dead.

use std::collections::HashMap;

use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, ast, ast::AstToken, ast::BinaryOp, ast::HasArgList, ast::HasName, ast::HasVisibility};

use crate::model::*;
use crate::provenance::{CrateInfo, Provenance};

pub struct FileIndex {
    pub path: String,
    pub src: String,
    pub lines: LineIndex,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
    /// Leaf name -> full path, from `use` declarations.
    pub imports: HashMap<String, String>,
    pub globs: Vec<String>,
    pub parse_errors: Vec<String>,
    /// Module path of this file within its crate, e.g. `["db", "pool"]`.
    pub module_path: Vec<String>,
}

impl FileIndex {
    pub fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id]
    }

    /// References that land on `id` — "find all references".
    pub fn callers_of(&self, id: SymbolId) -> Vec<&Reference> {
        self.references.iter().filter(|r| r.target() == Some(id)).collect()
    }

    /// References made from inside `id` — "what does this call".
    pub fn callees_of(&self, id: SymbolId) -> Vec<&Reference> {
        self.references.iter().filter(|r| r.from == Some(id)).collect()
    }

    pub fn callables(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter().filter(|s| s.kind.is_callable())
    }

    /// A reference worth showing. Every call site counts; bare value and type
    /// mentions only count when they actually point somewhere.
    pub fn is_interesting(&self, r: &Reference) -> bool {
        match r.kind {
            RefKind::Call | RefKind::MethodCall | RefKind::MacroCall => true,
            RefKind::ValueRef | RefKind::TypeRef => !matches!(r.to, Resolution::Unresolved { origin: None, .. }),
        }
    }
}

/// Index a single file with no crate context. Every non-local name lands in
/// `Provenance::Unknown` unless it is obviously `std` or an explicit
/// `crate::` path — use [`index_file_in`] when a `Cargo.toml` is available.
pub fn index_file(path: &str, src: &str) -> FileIndex {
    index_file_in(path, src, &CrateInfo::unknown(), Vec::new())
}

/// Index a file, classifying every outside name against `info`.
pub fn index_file_in(path: &str, src: &str, info: &CrateInfo, module_path: Vec<String>) -> FileIndex {
    let parsed = SourceFile::parse(src, Edition::Edition2021);
    let root = parsed.syntax_node();
    let lines = LineIndex::new(src);

    let parse_errors = parsed
        .errors()
        .iter()
        .map(|e| {
            let (l, c) = lines.line_col(src, e.range().start().into());
            format!("{}:{}: {}", l, c, e)
        })
        .collect();

    let (imports, globs) = collect_imports(&root);
    let symbols = collect_symbols(&root, src, &lines);
    let lookup = Lookup::build(&symbols);
    let references = collect_references(&root, src, &lines, &symbols, &lookup, &imports, &globs, info);

    FileIndex { path: path.to_string(), src: src.to_string(), lines, symbols, references, imports, globs, parse_errors, module_path }
}

// ---------------------------------------------------------------- imports

fn collect_imports(root: &SyntaxNode) -> (HashMap<String, String>, Vec<String>) {
    let mut imports = HashMap::new();
    let mut globs = Vec::new();
    for u in root.descendants().filter_map(ast::Use::cast) {
        if let Some(tree) = u.use_tree() {
            walk_use_tree(&tree, "", &mut imports, &mut globs);
        }
    }
    (imports, globs)
}

fn walk_use_tree(tree: &ast::UseTree, prefix: &str, imports: &mut HashMap<String, String>, globs: &mut Vec<String>) {
    let seg = tree.path().map(|p| p.syntax().text().to_string()).unwrap_or_default();
    // In `use crate::ast::{self, HasName}` the `self` leaf names the module
    // itself, so its path is the prefix — not `crate::ast::self`.
    let is_self = seg.trim() == "self";
    let full = match (prefix.is_empty(), seg.is_empty() || is_self) {
        (true, true) => return,
        (true, false) => seg.clone(),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix}::{seg}"),
    };

    if let Some(list) = tree.use_tree_list() {
        for child in list.use_trees() {
            walk_use_tree(&child, &full, imports, globs);
        }
    } else if tree.star_token().is_some() {
        globs.push(full);
    } else {
        let leaf = tree.rename().and_then(|r| r.name()).map(|n| n.text().to_string()).or_else(|| full.rsplit("::").next().map(str::to_string));
        if let Some(leaf) = leaf {
            if !leaf.is_empty() {
                imports.insert(leaf, full);
            }
        }
    }
}

// ------------------------------------------------------------ definitions

fn collect_symbols(root: &SyntaxNode, src: &str, lines: &LineIndex) -> Vec<Symbol> {
    let mut symbols: Vec<Symbol> = Vec::new();

    for node in root.descendants() {
        let (kind, name_node) = if let Some(f) = ast::Fn::cast(node.clone()) {
            let owner_kind = if owner_of(&node).is_some() { SymbolKind::Method } else { SymbolKind::Function };
            (owner_kind, f.name())
        } else if let Some(s) = ast::Struct::cast(node.clone()) {
            (SymbolKind::Struct, s.name())
        } else if let Some(e) = ast::Enum::cast(node.clone()) {
            (SymbolKind::Enum, e.name())
        } else if let Some(t) = ast::Trait::cast(node.clone()) {
            (SymbolKind::Trait, t.name())
        } else if let Some(t) = ast::TypeAlias::cast(node.clone()) {
            (SymbolKind::TypeAlias, t.name())
        } else if let Some(c) = ast::Const::cast(node.clone()) {
            (SymbolKind::Const, c.name())
        } else if let Some(s) = ast::Static::cast(node.clone()) {
            (SymbolKind::Static, s.name())
        } else if let Some(m) = ast::Module::cast(node.clone()) {
            (SymbolKind::Module, m.name())
        } else {
            continue;
        };

        let Some(name_node) = name_node else { continue };
        let name = name_node.text().to_string();

        let module_path = module_path_of(&node);
        let (owner, implements, owner_kind) = match owner_of(&node) {
            Some((o, i, k)) => (Some(o), i, Some(k)),
            None => (None, None, None),
        };

        let mut parts = module_path.clone();
        if let Some(o) = &owner {
            parts.push(o.clone());
        }
        parts.push(name.clone());
        let qualified = parts.join("::");

        let range = (node.text_range().start().into(), node.text_range().end().into());
        let (line, col) = lines.line_col(src, name_node.syntax().text_range().start().into());

        symbols.push(Symbol {
            id: symbols.len(),
            name,
            qualified,
            kind,
            owner,
            implements,
            owner_kind,
            module_path,
            visibility: visibility_of(&node),
            is_async: ast::Fn::cast(node.clone()).and_then(|f| f.async_token()).is_some(),
            signature: signature_of(&node),
            params: params_of(&node),
            ret: ret_of(&node),
            doc: doc_of(&node),
            loc: Loc { line, col, offset: range.0 },
            range,
        });
    }

    symbols
}

/// `(owner type, trait being implemented, which construct)` for the nearest
/// enclosing `impl`, or the trait name for a method declared inside a `trait`.
fn owner_of(node: &SyntaxNode) -> Option<(String, Option<String>, OwnerKind)> {
    for anc in node.ancestors().skip(1) {
        if let Some(imp) = ast::Impl::cast(anc.clone()) {
            let self_ty = imp.self_ty().map(|t| t.syntax().text().to_string()).unwrap_or_else(|| "?".into());
            let tr = imp.trait_().map(|t| t.syntax().text().to_string());
            return Some((self_ty, tr, OwnerKind::Impl));
        }
        if let Some(tr) = ast::Trait::cast(anc.clone()) {
            return Some((tr.name()?.text().to_string(), None, OwnerKind::TraitDecl));
        }
        // Stop at a module boundary: an item nested in a `mod` inside an
        // `impl` is not a method of that impl.
        if ast::Module::cast(anc.clone()).is_some() {
            return None;
        }
    }
    None
}

fn module_path_of(node: &SyntaxNode) -> Vec<String> {
    let mut path: Vec<String> = node.ancestors().skip(1).filter_map(ast::Module::cast).filter_map(|m| m.name()).map(|n| n.text().to_string()).collect();
    path.reverse();
    path
}

fn visibility_of(node: &SyntaxNode) -> Visibility {
    let vis_text = [
        ast::Fn::cast(node.clone()).and_then(|n| n.visibility()),
        ast::Struct::cast(node.clone()).and_then(|n| n.visibility()),
        ast::Enum::cast(node.clone()).and_then(|n| n.visibility()),
        ast::Trait::cast(node.clone()).and_then(|n| n.visibility()),
        ast::TypeAlias::cast(node.clone()).and_then(|n| n.visibility()),
        ast::Const::cast(node.clone()).and_then(|n| n.visibility()),
        ast::Static::cast(node.clone()).and_then(|n| n.visibility()),
        ast::Module::cast(node.clone()).and_then(|n| n.visibility()),
    ]
    .into_iter()
    .flatten()
    .next()
    .map(|v| v.syntax().text().to_string());

    match vis_text.as_deref() {
        None => Visibility::Private,
        Some(t) if t.contains("crate") || t.contains("super") || t.contains("self") => Visibility::Crate,
        Some(_) => Visibility::Public,
    }
}

/// The declaration line without its body — `fn foo(a: u32) -> bool`.
fn signature_of(node: &SyntaxNode) -> String {
    let full = node.text().to_string();
    let head = match ast::Fn::cast(node.clone()) {
        Some(f) => {
            let mut s = String::new();
            if f.async_token().is_some() {
                s.push_str("async ");
            }
            s.push_str("fn ");
            s.push_str(&f.name().map(|n| n.text().to_string()).unwrap_or_default());
            if let Some(p) = f.param_list() {
                s.push_str(&p.syntax().text().to_string());
            }
            if let Some(r) = f.ret_type() {
                s.push(' ');
                s.push_str(&r.syntax().text().to_string());
            }
            s
        }
        None => full.lines().next().unwrap_or("").trim().to_string(),
    };
    collapse_ws(&head)
}

/// Parameters of a function, receiver first. Empty for anything else.
fn params_of(node: &SyntaxNode) -> Vec<Param> {
    let Some(list) = ast::Fn::cast(node.clone()).and_then(|f| f.param_list()) else {
        return Vec::new();
    };
    let mut out = Vec::new();

    if let Some(receiver) = list.self_param() {
        out.push(Param {
            name: "self".to_string(),
            // `&self`, `&mut self`, `self: Box<Self>` — the whole thing is the
            // interesting part, so keep its text as the type.
            ty: collapse_ws(&receiver.syntax().text().to_string()),
        });
    }
    for p in list.params() {
        out.push(Param { name: p.pat().map(|pat| collapse_ws(&pat.syntax().text().to_string())).unwrap_or_else(|| "_".to_string()), ty: p.ty().map(|t| collapse_ws(&t.syntax().text().to_string())).unwrap_or_default() });
    }
    out
}

/// The return type as written, or `None` for `()`.
fn ret_of(node: &SyntaxNode) -> Option<String> {
    let ty = ast::Fn::cast(node.clone())?.ret_type()?.ty()?;
    Some(collapse_ws(&ty.syntax().text().to_string()))
}

/// The `///` doc comment attached to an item, with the prefixes stripped and
/// the lines joined. Returns `None` for an undocumented item.
fn doc_of(node: &SyntaxNode) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for child in node.children_with_tokens() {
        let Some(token) = child.as_token() else {
            // Doc comments precede the item's own syntax, so once a child node
            // appears there are none left to find.
            break;
        };
        if token.kind() != SyntaxKind::COMMENT {
            continue;
        }
        // `doc_comment` borrows from the token, so copy before it drops.
        if let Some(comment) = ast::Comment::cast(token.clone()) {
            if let Some(text) = comment.doc_comment() {
                lines.push(text.trim().to_string());
            }
        }
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// -------------------------------------------------------------- resolution

pub(crate) struct Lookup {
    /// Free functions by bare name.
    free_fns: HashMap<String, Vec<SymbolId>>,
    /// `(owner type, method name)` -> method.
    methods: HashMap<(String, String), SymbolId>,
    /// Method name -> every method with that name, any owner.
    methods_by_name: HashMap<String, Vec<SymbolId>>,
    /// Type-ish definitions by bare name.
    types: HashMap<String, SymbolId>,
    /// Anything, by its in-file qualified path.
    by_qualified: HashMap<String, SymbolId>,
}

impl Lookup {
    fn build(symbols: &[Symbol]) -> Self {
        let mut l = Lookup { free_fns: HashMap::new(), methods: HashMap::new(), methods_by_name: HashMap::new(), types: HashMap::new(), by_qualified: HashMap::new() };
        for s in symbols {
            // A `mod` is never what a call or a value names. Leaving it out
            // matters when a module and the function it holds share a name —
            // `mod load_settings;` next to `use load_settings::load_settings;` —
            // or the call binds to the module and is lost.
            if s.kind != SymbolKind::Module {
                l.by_qualified.insert(s.qualified.clone(), s.id);
            }
            match s.kind {
                SymbolKind::Function => {
                    l.free_fns.entry(s.name.clone()).or_default().push(s.id);
                }
                SymbolKind::Method => {
                    if let Some(owner) = &s.owner {
                        l.methods.insert((owner.clone(), s.name.clone()), s.id);
                    }
                    l.methods_by_name.entry(s.name.clone()).or_default().push(s.id);
                }
                SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait | SymbolKind::TypeAlias => {
                    l.types.insert(s.name.clone(), s.id);
                }
                _ => {}
            }
        }
        l
    }
}

fn from_ids(ids: &[SymbolId]) -> Option<Resolution> {
    match ids.len() {
        0 => None,
        1 => Some(Resolution::Local(ids[0])),
        _ => Some(Resolution::Ambiguous(ids.to_vec())),
    }
}

/// Names in scope without any `use`, so an unmatched head segment here means
/// the prelude rather than an unknown module.
const PRELUDE: &[&str] = &[
    "Some",
    "None",
    "Ok",
    "Err",
    "Option",
    "Result",
    "Vec",
    "String",
    "Box",
    "Default",
    "Iterator",
    "Into",
    "From",
    "Drop",
    "Clone",
    "Copy",
    "Send",
    "Sync",
    "Sized",
    "Fn",
    "FnMut",
    "FnOnce",
    "ToString",
    "AsRef",
    "AsMut",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Hash",
    "Debug",
    "Display",
];

/// Where an unresolved *path* probably lives: an explicit `use`, the prelude,
/// or — as a last guess — a glob import.
///
/// Only paths get an origin. A bare `.foo()` is deliberately left blank: which
/// `foo` it lands on depends on the receiver's type, and guessing from the
/// import list would attribute every `.iter()` to whichever glob came first.
fn external(kind: RefKind, path: &str, imports: &HashMap<String, String>, globs: &[String], info: &CrateInfo) -> Resolution {
    let path = strip_generics(path);
    let path = path.as_str();
    let head = path.split("::").next().unwrap_or(path);
    let origin = imports
        .get(head)
        .cloned()
        .or_else(|| imports.get(path).cloned())
        .or_else(|| PRELUDE.contains(&head).then(|| "std prelude".to_string()))
        // A glob import can only be guessed at, so restrict the guess to names
        // that follow the convention for what a glob brings in: types, traits,
        // enum variants and consts. A snake_case name in expression position is
        // overwhelmingly a local binding, and blaming a glob for it buries the
        // real findings under thousands of false ones.
        .or_else(|| looks_like_imported_item(head).then(|| globs.first().map(|g| format!("{g}::*  (via glob)"))).flatten());
    let provenance = info.classify(origin.as_deref(), path, kind == RefKind::MacroCall);
    Resolution::Unresolved { origin, provenance }
}

/// Does this name follow Rust's convention for something a glob would import?
/// `Resolution` and `MAX_DEPTH` do; `prev_node` does not.
fn looks_like_imported_item(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

/// Drop anything from the first `<` onwards.
fn strip_generics(path: &str) -> String {
    match path.find('<') {
        Some(i) => path[..i].trim_end_matches(':').trim().to_string(),
        None => path.to_string(),
    }
}

/// A short, readable stand-in for a method receiver. The full text of a long
/// call chain is useless in a report, so collapse it to its tail.
fn short_receiver(expr: &ast::Expr) -> String {
    let text = |n: &SyntaxNode| truncate(&collapse_ws(&n.text().to_string()), 22);
    match expr {
        ast::Expr::MethodCallExpr(m) => format!("\u{2026}.{}()", m.name_ref().map(|n| n.text().to_string()).unwrap_or_default()),
        ast::Expr::CallExpr(c) => match c.expr() {
            Some(callee) => format!("{}()", text(callee.syntax())),
            None => "\u{2026}".to_string(),
        },
        ast::Expr::PathExpr(p) => text(p.syntax()),
        ast::Expr::FieldExpr(f) => text(f.syntax()),
        ast::Expr::TryExpr(t) => match t.expr() {
            Some(inner) => format!("{}?", short_receiver(&inner)),
            None => "\u{2026}".to_string(),
        },
        ast::Expr::AwaitExpr(a) => match a.expr() {
            Some(inner) => format!("{}.await", short_receiver(&inner)),
            None => "\u{2026}".to_string(),
        },
        other => text(other.syntax()),
    }
}

/// Cut to `max` characters on a char boundary.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}\u{2026}")
}

// -------------------------------------------------------------- references

/// Wrapper used to re-parse a macro's arguments as real expressions. A macro
/// body is an unparsed token tree, so `format!("{}", esc(x))` hides the call to
/// `esc` from the syntax tree entirely. Splicing the arguments into a tuple
/// makes them parseable: `("{}", esc(x))` is a valid expression.
const MACRO_PREFIX: &str = "fn __macro_args__() { (";
const MACRO_SUFFIX: &str = ") ; }";

/// How deep to follow macros nested inside macro arguments.
const MAX_MACRO_DEPTH: usize = 3;

/// Maps offsets in a re-parsed macro body back to the original file.
#[derive(Clone, Copy)]
struct Shift {
    /// Offset in the original file where the spliced text began.
    origin: usize,
    /// Length of the spliced text, to reject out-of-range offsets.
    len: usize,
}

#[derive(Clone, Copy, Default)]
struct Ctx {
    shift: Option<Shift>,
    /// Inside a macro, every reference belongs to whatever enclosed the call.
    enclosing: Option<SymbolId>,
    via_macro: bool,
}

impl Ctx {
    /// Offset in the original file, or `None` if it falls in the wrapper.
    fn to_original(self, offset: usize) -> Option<usize> {
        match self.shift {
            None => Some(offset),
            Some(shift) => {
                let inner = offset.checked_sub(MACRO_PREFIX.len())?;
                (inner <= shift.len).then_some(shift.origin + inner)
            }
        }
    }
}

struct Collector<'a> {
    src: &'a str,
    lines: &'a LineIndex,
    symbols: &'a [Symbol],
    lookup: &'a Lookup,
    imports: &'a HashMap<String, String>,
    globs: &'a [String],
    info: &'a CrateInfo,
    /// Definition start offset -> its id, to find the enclosing symbol.
    by_offset: HashMap<usize, SymbolId>,
    out: Vec<Reference>,
}

#[allow(clippy::too_many_arguments)]
fn collect_references(root: &SyntaxNode, src: &str, lines: &LineIndex, symbols: &[Symbol], lookup: &Lookup, imports: &HashMap<String, String>, globs: &[String], info: &CrateInfo) -> Vec<Reference> {
    let mut collector = Collector { src, lines, symbols, lookup, imports, globs, info, by_offset: symbols.iter().map(|s| (s.range.0, s.id)).collect(), out: Vec::new() };
    collector.walk(root, Ctx::default(), 0);
    collector.out
}

impl Collector<'_> {
    fn walk(&mut self, root: &SyntaxNode, ctx: Ctx, depth: usize) {
        for node in root.descendants() {
            if let Some(mac) = ast::MacroCall::cast(node.clone()) {
                self.record(&node, ctx);
                if depth < MAX_MACRO_DEPTH {
                    self.walk_macro_args(&mac, ctx, depth);
                }
                continue;
            }
            self.record(&node, ctx);
        }
    }

    /// Re-parse a macro's token tree and harvest the calls hiding in it.
    fn walk_macro_args(&mut self, mac: &ast::MacroCall, ctx: Ctx, depth: usize) {
        let Some(tt) = mac.token_tree() else { return };
        let text = tt.syntax().text().to_string();
        // Drop the outer delimiter pair; what remains is the argument list.
        let mut chars = text.chars();
        let opens = matches!(chars.next(), Some('(') | Some('[') | Some('{'));
        if !opens || text.len() < 2 {
            return;
        }
        let inner = &text[1..text.len() - 1];
        if inner.trim().is_empty() {
            return;
        }

        let tt_start: usize = tt.syntax().text_range().start().into();
        let Some(origin) = ctx.to_original(tt_start + 1) else {
            return;
        };

        let wrapped = format!("{MACRO_PREFIX}{inner}{MACRO_SUFFIX}");
        let parsed = SourceFile::parse(&wrapped, Edition::Edition2021);

        // The enclosing symbol comes from the original tree: the re-parsed
        // snippet has only the synthetic wrapper function.
        let enclosing = ctx.enclosing.or_else(|| self.enclosing_symbol(mac.syntax()));

        let inner_ctx = Ctx { shift: Some(Shift { origin, len: inner.len() }), enclosing, via_macro: true };
        self.walk(&parsed.syntax_node(), inner_ctx, depth + 1);
    }

    /// Record `node` if it is a reference of interest.
    fn record(&mut self, node: &SyntaxNode, ctx: Ctx) {
        let mut recv_is_self = false;
        let (kind, name, path) = if let Some(call) = ast::CallExpr::cast(node.clone()) {
            let Some(ast::Expr::PathExpr(pe)) = call.expr() else {
                return;
            };
            let Some(p) = pe.path() else { return };
            let text = path_text(&p);
            (RefKind::Call, last_segment(&p, &text), text)
        } else if let Some(mc) = ast::MethodCallExpr::cast(node.clone()) {
            let Some(nr) = mc.name_ref() else { return };
            let receiver = mc.receiver();
            // `self.foo()` is a call on my own type. `self.field.foo()` is not
            // — the receiver there is the field, whose type we do not know.
            recv_is_self = receiver.as_ref().is_some_and(|r| {
                matches!(r, ast::Expr::PathExpr(p)
                    if p.syntax().text().to_string().trim() == "self")
            });
            let recv = receiver.map(|r| short_receiver(&r)).unwrap_or_default();
            let name = nr.text().to_string();
            (RefKind::MethodCall, name.clone(), format!("{recv}.{name}"))
        } else if let Some(mac) = ast::MacroCall::cast(node.clone()) {
            let Some(p) = mac.path() else { return };
            let text = path_text(&p);
            (RefKind::MacroCall, last_segment(&p, &text), text)
        } else if let Some(pe) = ast::PathExpr::cast(node.clone()) {
            // Skip the callee of a call — recorded above as the call itself.
            if node.parent().is_some_and(|p| ast::CallExpr::can_cast(p.kind())) {
                return;
            }
            let Some(p) = pe.path() else { return };
            let text = path_text(&p);
            (RefKind::ValueRef, last_segment(&p, &text), text)
        } else if let Some(pt) = ast::PathType::cast(node.clone()) {
            let Some(p) = pt.path() else { return };
            let text = path_text(&p);
            (RefKind::TypeRef, last_segment(&p, &text), text)
        } else {
            return;
        };

        let start: usize = node.text_range().start().into();
        let Some(offset) = ctx.to_original(start) else {
            // Sits in the synthetic wrapper, not in the user's code.
            return;
        };

        let args = if matches!(kind, RefKind::Call | RefKind::MethodCall) { args_of(node) } else { Vec::new() };
        let binding = binding_of(node);
        // Only meaningful for the code as written; a reference recovered from
        // a macro body has no reliable position in the original tree.
        let context = if ctx.via_macro { Vec::new() } else { control_context(node, self.src, self.lines) };

        let from = ctx.enclosing.or_else(|| self.enclosing_symbol(node));
        let owner = from.and_then(|id| self.symbols[id].owner.clone());
        let to = resolve(kind, &name, &path, owner.as_deref(), recv_is_self, self.lookup, self.imports, self.globs, self.info);
        let (line, col) = self.lines.line_col(self.src, offset);

        self.out.push(Reference { name, path, kind, from, to, args, binding, context, loc: Loc { line, col, offset }, by_name: false, via_macro: ctx.via_macro });
    }

    /// Nearest enclosing definition that could own a reference.
    fn enclosing_symbol(&self, node: &SyntaxNode) -> Option<SymbolId> {
        for anc in node.ancestors().skip(1) {
            if ast::Fn::cast(anc.clone()).is_some() || ast::Const::cast(anc.clone()).is_some() || ast::Static::cast(anc.clone()).is_some() {
                let start: usize = anc.text_range().start().into();
                if let Some(id) = self.by_offset.get(&start) {
                    return Some(*id);
                }
            }
        }
        None
    }
}

/// The arguments written at a call site, each collapsed to one line.
///
/// `ArgList` has no typed accessor for its arguments, so the expression
/// children are read directly; the parentheses are tokens, not nodes, and are
/// skipped for free.
fn args_of(node: &SyntaxNode) -> Vec<String> {
    let arg_list = ast::CallExpr::cast(node.clone()).and_then(|c| c.arg_list()).or_else(|| ast::MethodCallExpr::cast(node.clone()).and_then(|m| m.arg_list()));
    let Some(arg_list) = arg_list else {
        return Vec::new();
    };
    arg_list
        .syntax()
        .children()
        .filter_map(ast::Expr::cast)
        // Kept whole. Shortening here would lose the text for good; the page
        // clips long arguments visually and expands them on click.
        .map(|e| collapse_ws(&e.syntax().text().to_string()))
        .collect()
}

/// The loops, branches and match arms enclosing a node, outermost first.
///
/// Stops at the function boundary: the context of a call is the shape of the
/// body it sits in, not of everything above it.
fn control_context(node: &SyntaxNode, src: &str, lines: &LineIndex) -> Vec<ControlFrame> {
    let mut frames: Vec<ControlFrame> = Vec::new();
    let mut child = node.clone();

    for anc in node.ancestors().skip(1) {
        if ast::Fn::cast(anc.clone()).is_some() {
            break;
        }
        let frame = match anc.kind() {
            SyntaxKind::FOR_EXPR => ast::ForExpr::cast(anc.clone()).map(|e| {
                let pat = e.pat().map(|p| collapse_ws(&p.syntax().text().to_string())).unwrap_or_else(|| "_".into());
                let iter = e.iterable().map(|i| collapse_ws(&i.syntax().text().to_string())).unwrap_or_default();
                (ControlKind::For, format!("for {pat} in {iter}"))
            }),
            SyntaxKind::WHILE_EXPR => ast::WhileExpr::cast(anc.clone()).map(|e| {
                let cond = e.condition().map(|c| collapse_ws(&c.syntax().text().to_string())).unwrap_or_default();
                (ControlKind::While, format!("while {cond}"))
            }),
            SyntaxKind::LOOP_EXPR => Some((ControlKind::Loop, "loop".to_string())),
            SyntaxKind::IF_EXPR => ast::IfExpr::cast(anc.clone()).map(|e| {
                let cond = e.condition().map(|c| collapse_ws(&c.syntax().text().to_string())).unwrap_or_default();
                // Which side of the `if` are we on?
                let in_else = match e.else_branch() {
                    Some(ast::ElseBranch::Block(b)) => b.syntax().text_range().contains_range(child.text_range()),
                    Some(ast::ElseBranch::IfExpr(i)) => i.syntax().text_range().contains_range(child.text_range()),
                    None => false,
                };
                if in_else {
                    // An `else if` records its own condition on the next turn
                    // through the loop, so this frame is the plain `else`.
                    (ControlKind::Else, format!("else of: if {cond}"))
                } else {
                    (ControlKind::If, format!("if {cond}"))
                }
            }),
            SyntaxKind::MATCH_ARM => ast::MatchArm::cast(anc.clone()).map(|a| {
                let pat = a.pat().map(|p| collapse_ws(&p.syntax().text().to_string())).unwrap_or_else(|| "_".into());
                (ControlKind::MatchArm, pat)
            }),
            SyntaxKind::CLOSURE_EXPR => Some((ControlKind::Closure, "closure".to_string())),
            _ => None,
        };

        if let Some((kind, label)) = frame {
            let offset: usize = anc.text_range().start().into();
            let (line, _) = lines.line_col(src, offset);
            frames.push(ControlFrame { kind, label, offset, line });
        }
        child = anc;
    }

    frames.reverse();
    frames
}

/// How far to climb looking for the binding a call's value flows into.
const MAX_BINDING_CLIMB: usize = 8;

/// The variable a call's result is stored in, if any.
///
/// The value is followed up through constructs that pass it along unchanged —
/// `?`, `.await`, parentheses, casts, and postfix calls like `.unwrap()` — so
/// `let ws = Workspace::load(p)?;` credits `ws` to `Workspace::load`. A call
/// used as an argument, a condition, or a match scrutinee is stored nowhere and
/// reports `None` rather than borrowing an unrelated name.
fn binding_of(node: &SyntaxNode) -> Option<String> {
    let mut cur = node.clone();
    for _ in 0..MAX_BINDING_CLIMB {
        let parent = cur.parent()?;
        match parent.kind() {
            SyntaxKind::LET_STMT => {
                let stmt = ast::LetStmt::cast(parent)?;
                // Only the initializer stores; a `let ... else` block does not.
                if stmt.initializer()?.syntax().text_range() != cur.text_range() {
                    return None;
                }
                return stmt.pat().map(|p| collapse_ws(&p.syntax().text().to_string()));
            }
            SyntaxKind::BIN_EXPR => {
                let bin = ast::BinExpr::cast(parent)?;
                // `x = foo()` and `x += foo()` store; `x == foo()` does not.
                if !matches!(bin.op_details()?.1, BinaryOp::Assignment { .. }) {
                    return None;
                }
                if bin.rhs()?.syntax().text_range() != cur.text_range() {
                    return None;
                }
                return bin.lhs().map(|l| collapse_ws(&l.syntax().text().to_string()));
            }
            // The value passes through these unchanged.
            SyntaxKind::TRY_EXPR | SyntaxKind::AWAIT_EXPR | SyntaxKind::PAREN_EXPR | SyntaxKind::CAST_EXPR | SyntaxKind::REF_EXPR => cur = parent,
            // `foo().unwrap()` — only the receiver's value flows onward.
            SyntaxKind::METHOD_CALL_EXPR => {
                let mc = ast::MethodCallExpr::cast(parent.clone())?;
                if mc.receiver()?.syntax().text_range() != cur.text_range() {
                    return None;
                }
                cur = parent;
            }
            _ => return None,
        }
    }
    None
}

/// The last segment of a path, falling back to the whole text.
fn last_segment(path: &ast::Path, text: &str) -> String {
    path.segment().and_then(|s| s.name_ref()).map(|n| n.text().to_string()).unwrap_or_else(|| text.to_string())
}

/// A path's segments joined by `::`, without generic arguments.
///
/// The raw text of `HashMap<String, String>` matches no import and resolves to
/// nothing; `HashMap` does. Nothing is lost by dropping the arguments — each is
/// its own reference in the tree and gets recorded separately.
fn path_text(path: &ast::Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cursor = Some(path.clone());
    while let Some(p) = cursor {
        if let Some(seg) = p.segment() {
            match seg.name_ref() {
                Some(nr) => parts.push(nr.text().to_string()),
                // `crate`, `self`, `super` and `<T as Trait>` anchors have no
                // name ref of their own.
                None => parts.push(collapse_ws(&seg.syntax().text().to_string())),
            }
        }
        cursor = p.qualifier();
    }
    parts.reverse();
    parts.join("::")
}

#[allow(clippy::too_many_arguments)]
fn resolve(kind: RefKind, name: &str, path: &str, enclosing_owner: Option<&str>, recv_is_self: bool, lookup: &Lookup, imports: &HashMap<String, String>, globs: &[String], info: &CrateInfo) -> Resolution {
    // `Self::foo` means the enclosing impl's type.
    let path = match (path.strip_prefix("Self::"), enclosing_owner) {
        (Some(rest), Some(owner)) => format!("{owner}::{rest}"),
        _ => path.to_string(),
    };

    match kind {
        RefKind::MethodCall => {
            // A bare `self` receiver pins the type: look on the enclosing impl.
            if recv_is_self {
                if let Some(owner) = enclosing_owner {
                    if let Some(id) = lookup.methods.get(&(owner.to_string(), name.to_string())) {
                        return Resolution::Local(*id);
                    }
                }
            }
            // Otherwise the receiver's type is unknown; match by name alone.
            if let Some(ids) = lookup.methods_by_name.get(name) {
                if let Some(r) = from_ids(ids) {
                    return r;
                }
            }
            // Nothing local matches. The receiver's type is unknown, so there
            // is no path to classify — say so rather than guessing.
            let _ = (imports, globs, info);
            Resolution::Unresolved { origin: None, provenance: Provenance::Unknown }
        }
        RefKind::Call | RefKind::ValueRef => {
            // Fully qualified within this file, e.g. `mymod::helper`.
            if let Some(id) = lookup.by_qualified.get(&path) {
                return Resolution::Local(*id);
            }
            let segs: Vec<&str> = path.split("::").collect();
            if segs.len() >= 2 {
                // `Type::assoc()` — the segment before the name names the type.
                let ty = segs[segs.len() - 2];
                if let Some(id) = lookup.methods.get(&(ty.to_string(), name.to_string())) {
                    return Resolution::Local(*id);
                }
                // `Enum::Variant` — points at the enum itself.
                if segs.len() == 2 {
                    if let Some(id) = lookup.types.get(segs[0]) {
                        return Resolution::Local(*id);
                    }
                }
                return external(kind, &path, imports, globs, info);
            }
            // Bare name: a free function, else a tuple-struct constructor.
            if let Some(ids) = lookup.free_fns.get(name) {
                if let Some(r) = from_ids(ids) {
                    return r;
                }
            }
            if let Some(id) = lookup.types.get(name) {
                return Resolution::Local(*id);
            }
            external(kind, &path, imports, globs, info)
        }
        RefKind::TypeRef => {
            if let Some(id) = lookup.types.get(name) {
                return Resolution::Local(*id);
            }
            external(kind, &path, imports, globs, info)
        }
        RefKind::MacroCall => external(kind, &path, imports, globs, info),
    }
}
