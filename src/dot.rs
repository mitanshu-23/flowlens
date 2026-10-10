//! Graphviz output.
//!
//! Two views, because they answer different questions:
//!
//! * [`emit_call_graph`] — functions and the calls between them, clustered by
//!   file. Cross-file calls are drawn heavier than local ones, since those are
//!   the couplings that matter when you go to change something.
//! * [`emit_file_graph`] — one node per file, edges weighted by how many
//!   references cross. The readable view once a crate outgrows one screen.
//!
//! Both default to user-written code only. Drawing every `.iter()` and
//! `HashMap::new` buries the structure you are looking for.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::*;
use crate::provenance::Provenance;
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Also draw calls into `std` and dependencies, collapsed per origin.
    pub include_library: bool,
    /// Put each function's doc summary on its node, so the graph says what the
    /// code *does* and not only what it is called. Costs width.
    pub show_docs: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { include_library: false, show_docs: true }
    }
}

// ------------------------------------------------------------- call graph

pub fn emit_call_graph(ws: &Workspace, opts: Options) -> String {
    let mut out = String::new();
    let p = &mut out;

    preamble(p, "call_graph", &format!("call graph — {} ({} file(s))", ws.info.name.clone().unwrap_or_else(|| "crate".into()), ws.files.len()));

    // ---- one cluster per file, subdivided by impl block
    for (fid, f) in ws.files.iter().enumerate() {
        let callables: Vec<&Symbol> = f.callables().collect();
        if callables.is_empty() {
            continue;
        }
        let module = if f.module_path.is_empty() { "crate".to_string() } else { format!("crate::{}", f.module_path.join("::")) };
        push(p, &format!("    subgraph cluster_file_{fid} {{"));
        push(p, &format!("        label=\"{}\\n{}\";", esc(&module), esc(&file_name(&f.path))));
        push(p, "        style=\"rounded,filled\"; fillcolor=\"#F7F9FB\"; color=\"#94A3B8\"; fontsize=13;");

        let mut by_owner: BTreeMap<Option<String>, Vec<&Symbol>> = BTreeMap::new();
        for s in callables {
            by_owner.entry(s.owner.clone()).or_default().push(s);
        }
        for (owner, syms) in &by_owner {
            let indent = match owner {
                Some(o) => {
                    push(p, &format!("        subgraph cluster_o_{fid}_{} {{", sanitize(o)));
                    push(p, &format!("            label=\"impl {}\";", esc(o)));
                    push(p, "            style=rounded; color=\"#CBD5E1\"; fontsize=11;");
                    "            "
                }
                None => "        ",
            };
            for s in syms {
                push(p, &format!("{indent}{} [label=\"{}\", fillcolor=\"{}\", color=\"{}\"{}, tooltip=\"{}\"];", node_id(fid, s.id), esc(&node_label(s, opts.show_docs)), fill_for(s), stroke_for(s), if s.name == "main" { ", penwidth=2" } else { "" }, esc(&tooltip_for(s))));
            }
            if owner.is_some() {
                push(p, "        }");
            }
        }
        push(p, "    }");
        push(p, "");
    }

    // ---- library origins, only on request
    let mut lib_names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if opts.include_library {
        for f in &ws.files {
            for r in &f.references {
                if r.from.is_none() || !f.is_interesting(r) {
                    continue;
                }
                let Resolution::Unresolved { origin, provenance } = &r.to else {
                    continue;
                };
                if *provenance != Provenance::ThirdParty {
                    continue;
                }
                let key = trim_origin(origin.as_deref().unwrap_or("std"));
                lib_names.entry(key).or_default().insert(r.name.clone());
            }
        }
        if !lib_names.is_empty() {
            push(p, "    subgraph cluster_library {");
            push(p, "        label=\"std + dependencies (not your code)\";");
            push(p, "        style=\"rounded,dashed\"; color=\"#94A3B8\"; fontsize=12;");
            for (origin, names) in &lib_names {
                let shown: Vec<&str> = names.iter().take(4).map(String::as_str).collect();
                let more = names.len() - shown.len();
                let mut label = format!("{origin}\n{} name(s)\n", names.len());
                label.push_str(&shown.join(", "));
                if more > 0 {
                    label.push_str(&format!(", +{more}"));
                }
                push(p, &format!("        lib_{} [label=\"{}\", shape=note, fillcolor=\"#F1F5F9\", color=\"#94A3B8\"];", sanitize(origin), esc(&label)));
            }
            push(p, "    }");
            push(p, "");
        }
    }

    // ---- edges
    let mut same: BTreeMap<(GlobalId, GlobalId), usize> = BTreeMap::new();
    let mut cross: BTreeMap<(GlobalId, GlobalId), usize> = BTreeMap::new();
    let mut amb: BTreeMap<(GlobalId, GlobalId), usize> = BTreeMap::new();
    let mut lib: BTreeMap<(GlobalId, String), usize> = BTreeMap::new();

    for (fid, f) in ws.files.iter().enumerate() {
        for r in &f.references {
            let Some(from_sym) = r.from else { continue };
            if !f.symbols[from_sym].kind.is_callable() {
                continue;
            }
            let from = GlobalId { file: fid, sym: from_sym };
            match &r.to {
                Resolution::Local(sym) => {
                    if f.symbols[*sym].kind.is_callable() {
                        *same.entry((from, GlobalId { file: fid, sym: *sym })).or_default() += 1;
                    }
                }
                Resolution::CrossFile(g) => {
                    if ws.symbol(*g).kind.is_callable() {
                        *cross.entry((from, *g)).or_default() += 1;
                    }
                }
                Resolution::Ambiguous(ids) => {
                    for sym in ids {
                        *amb.entry((from, GlobalId { file: fid, sym: *sym })).or_default() += 1;
                    }
                }
                Resolution::Unresolved { origin, provenance } => {
                    if !opts.include_library || *provenance != Provenance::ThirdParty {
                        continue;
                    }
                    if !f.is_interesting(r) {
                        continue;
                    }
                    let key = trim_origin(origin.as_deref().unwrap_or("std"));
                    *lib.entry((from, key)).or_default() += 1;
                }
            }
        }
    }

    for ((from, to), n) in &same {
        let mut attrs = Vec::new();
        if from == to {
            attrs.push("style=bold".to_string());
            attrs.push("color=\"#B5179E\"".to_string());
            attrs.push(format!("label=\"recursive{}\"", times(*n)));
        } else {
            attrs.push("color=\"#2563EB\"".to_string());
            attrs.push(format!("penwidth={:.1}", 1.0 + (*n as f64).log10()));
            if *n > 1 {
                attrs.push(format!("label=\"{}\"", times(*n).trim()));
            }
        }
        edge(p, from, to, &attrs);
    }
    // Cross-file calls are the couplings worth seeing first.
    for ((from, to), n) in &cross {
        let attrs = vec!["color=\"#EA580C\"".to_string(), format!("penwidth={:.1}", 1.6 + (*n as f64).log10()), format!("label=\"{}\"", times(*n).trim())];
        edge(p, from, to, &attrs);
    }
    for ((from, to), n) in &amb {
        let attrs = vec!["style=dotted".to_string(), "color=\"#7C3AED\"".to_string(), format!("label=\"?{}\"", times(*n))];
        edge(p, from, to, &attrs);
    }
    for ((from, origin), n) in &lib {
        push(p, &format!("    {} -> lib_{} [style=dashed, color=\"#CBD5E1\"{}];", node_id(from.file, from.sym), sanitize(origin), if *n > 1 { format!(", label=\"{}\"", times(*n).trim()) } else { String::new() }));
    }

    legend(p, ws.files.len() > 1);
    push(p, "}");
    out
}

// ------------------------------------------------------------- file graph

/// One node per file, edges weighted by crossing references.
pub fn emit_file_graph(ws: &Workspace) -> String {
    let mut out = String::new();
    let p = &mut out;

    preamble(p, "file_graph", &format!("file dependencies — {}", ws.info.name.clone().unwrap_or_else(|| "crate".into())));
    push(p, "    node [shape=box, style=\"filled,rounded\"];");
    push(p, "");

    let edges = ws.file_dependencies();
    let mut in_deg: BTreeMap<FileId, usize> = BTreeMap::new();
    for ((_, to), n) in &edges {
        *in_deg.entry(*to).or_default() += n;
    }

    for (fid, f) in ws.files.iter().enumerate() {
        let module = if f.module_path.is_empty() { "crate".to_string() } else { format!("crate::{}", f.module_path.join("::")) };
        let defs = f.callables().count();
        // Depth of the module tree gives the fill, so leaves read differently
        // from the root without needing a legend entry per level.
        let fill = match f.module_path.len() {
            0 => "#DBEAFE",
            1 => "#E0F2FE",
            _ => "#F0F9FF",
        };
        push(p, &format!("    file_{fid} [label=\"{}\\n{}\\n{} fn · {} incoming\", fillcolor=\"{}\", color=\"#2563EB\"];", esc(&module), esc(&file_name(&f.path)), defs, in_deg.get(&fid).copied().unwrap_or(0), fill));
    }
    push(p, "");

    if edges.is_empty() {
        push(p, "    empty [label=\"no cross-file references resolved\", shape=note, fillcolor=\"#FEF3C7\", color=\"#D97706\"];");
    }
    for ((from, to), n) in &edges {
        push(p, &format!("    file_{from} -> file_{to} [label=\"{n}\", penwidth={:.1}, color=\"#EA580C\"];", 1.0 + (*n as f64).log10() * 1.6));
    }

    push(p, "}");
    out
}

// ---------------------------------------------------------------- plumbing

fn preamble(p: &mut String, name: &str, title: &str) {
    push(p, &format!("digraph {name} {{"));
    push(p, "    rankdir=LR;");
    push(p, "    ranksep=1.1;");
    push(p, "    nodesep=0.35;");
    push(p, "    compound=true;");
    push(p, "    graph [fontname=\"Helvetica\", labelloc=t, fontsize=17];");
    push(p, "    node  [fontname=\"Helvetica\", fontsize=11, style=\"filled,rounded\", shape=box, margin=\"0.16,0.09\"];");
    push(p, "    edge  [fontname=\"Helvetica\", fontsize=9, arrowsize=0.7];");
    push(p, &format!("    label=\"{}\";", esc(title)));
    push(p, "");
}

fn legend(p: &mut String, multi_file: bool) {
    push(p, "");
    push(p, "    subgraph cluster_legend {");
    push(p, "        label=\"legend\"; style=rounded; color=\"#CBD5E1\"; fontsize=12;");
    push(p, "        rankdir=LR;");
    // Each sample edge gets its own pair of nodes, so nothing has to serve as
    // both a caption and an endpoint.
    push(p, "        lg_pub  [label=\"pub fn\",      fillcolor=\"#D7F5DD\", color=\"#16A34A\"];");
    push(p, "        lg_priv [label=\"private fn\",  fillcolor=\"#E8EDF2\", color=\"#94A3B8\"];");
    push(p, "        lg_main [label=\"entry point\", fillcolor=\"#DBEAFE\", color=\"#2563EB\", penwidth=2];");
    push(p, "        lg_pub -> lg_priv [color=\"#2563EB\", label=\"calls\"];");
    if multi_file {
        push(p, "        lg_x [label=\"in another file\", fillcolor=\"#E8EDF2\", color=\"#94A3B8\"];");
        push(p, "        lg_priv -> lg_x [color=\"#EA580C\", penwidth=1.6, label=\"calls across files\"];");
        push(p, "        lg_y [label=\"one of several\", fillcolor=\"#E8EDF2\", color=\"#94A3B8\"];");
        push(p, "        lg_x -> lg_y [style=dotted, color=\"#7C3AED\", label=\"name is ambiguous\"];");
    } else {
        push(p, "        lg_y [label=\"one of several\", fillcolor=\"#E8EDF2\", color=\"#94A3B8\"];");
        push(p, "        lg_priv -> lg_y [style=dotted, color=\"#7C3AED\", label=\"name is ambiguous\"];");
    }
    push(p, "        lg_main -> lg_pub [style=invis];");
    push(p, "    }");
}

fn edge(p: &mut String, from: &GlobalId, to: &GlobalId, attrs: &[String]) {
    push(p, &format!("    {} -> {} [{}];", node_id(from.file, from.sym), node_id(to.file, to.sym), attrs.join(", ")));
}

fn push(buf: &mut String, line: &str) {
    buf.push_str(line);
    buf.push('\n');
}

/// `" ×3"` for repeats, empty for a single occurrence.
fn times(n: usize) -> String {
    if n > 1 { format!(" \u{d7}{n}") } else { String::new() }
}

fn node_id(file: FileId, sym: SymbolId) -> String {
    format!("f{file}n{sym}")
}

fn node_label(s: &Symbol, show_docs: bool) -> String {
    let mut l = String::new();
    if s.is_async {
        l.push_str("async ");
    }
    match &s.owner {
        Some(o) => l.push_str(&format!("{o}::{}", s.name)),
        None => l.push_str(&s.name),
    }
    l.push_str(&format!("\nL{}", s.loc.line));

    if show_docs {
        if let Some(summary) = s.doc_summary() {
            // A rule then the summary, so the name stays the thing you read
            // first and the description reads as annotation.
            l.push_str("\n———\n");
            l.push_str(&wrap(&summary, 30, 3));
        }
    }
    l
}

/// Signature plus the full doc comment, shown on hover in SVG output. The node
/// only has room for a summary; this is where the rest goes.
fn tooltip_for(s: &Symbol) -> String {
    match &s.doc {
        Some(doc) => format!("{}\n\n{}", s.signature, doc),
        None => s.signature.clone(),
    }
}

/// Break `text` into lines of at most `width` characters, at most `max_lines`
/// of them, with an ellipsis if it does not fit.
fn wrap(text: &str, width: usize, max_lines: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
            if lines.len() == max_lines {
                break;
            }
        }
    }
    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }
    if lines.len() == max_lines && text.split_whitespace().count() > 0 {
        // Mark truncation only when something was actually dropped.
        let shown: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
        if shown < text.split_whitespace().count() {
            lines[max_lines - 1].push('…');
        }
    }
    lines.join("\n")
}

fn fill_for(s: &Symbol) -> &'static str {
    if s.name == "main" {
        return "#DBEAFE";
    }
    match s.visibility {
        Visibility::Public => "#D7F5DD",
        Visibility::Crate => "#FEF3C7",
        Visibility::Private => "#E8EDF2",
    }
}

fn stroke_for(s: &Symbol) -> &'static str {
    if s.name == "main" {
        return "#2563EB";
    }
    match s.visibility {
        Visibility::Public => "#16A34A",
        Visibility::Crate => "#D97706",
        Visibility::Private => "#94A3B8",
    }
}

/// `std::collections::HashSet` -> `std::collections`; keeps the node count sane.
fn trim_origin(origin: &str) -> String {
    let parts: Vec<&str> = origin.split("::").collect();
    if parts.len() <= 1 {
        return origin.to_string();
    }
    parts[..parts.len() - 1].join("::")
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}
