//! Human-readable rendering of a [`Workspace`].
//!
//! The organising principle is the split between code the user wrote and code
//! they merely depend on. A call to `normalize()` in a sibling module tells you
//! something about the design; a call to `.trim()` does not. So user-defined
//! calls get the detailed, per-call-site treatment and everything from `std` or
//! a dependency is collapsed into a summary.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::model::*;
use crate::provenance::Provenance;
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Spell out calls into `std` and dependencies instead of summarising.
    pub show_library: bool,
    /// List every definition per file, not just the counts.
    pub show_outline: bool,
    /// Print each definition's doc summary beneath it in the outline.
    pub show_docs: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { show_library: false, show_outline: true, show_docs: true }
    }
}

pub fn print_report(ws: &Workspace, opts: Options) {
    let multi = ws.files.len() > 1;

    header(ws);
    if multi {
        files_table(ws);
        interlinking(ws);
    }
    if opts.show_outline {
        outline(ws, multi, opts.show_docs);
    }
    user_calls(ws);
    callers(ws);
    entry_points(ws);
    library_summary(ws, opts.show_library);
    diagnostics(ws);
}

fn rule(title: &str) {
    println!("\n{}", title);
    println!("{}", "─".repeat(title.chars().count().max(24)));
}

// ------------------------------------------------------------------ header

fn header(ws: &Workspace) {
    let defs: usize = ws.files.iter().map(|f| f.symbols.len()).sum();
    let callables: usize = ws.files.iter().map(|f| f.callables().count()).sum();
    let name = ws.info.name.clone().unwrap_or_else(|| "(unknown crate)".into());

    println!("═══ {} ═══", name);
    println!("{} file(s) · {} definition(s) · {} callable(s)", ws.files.len(), defs, callables);
    if ws.files.len() > 1 {
        println!("{} cross-file reference(s) linked ({} by unique method name)", ws.linked, ws.linked_by_name);
    }
    let public: Vec<&Symbol> = ws.files.iter().flat_map(|f| f.symbols.iter()).filter(|s| s.kind.is_callable() && s.visibility == Visibility::Public).collect();
    if !public.is_empty() {
        let documented = public.iter().filter(|s| s.is_documented()).count();
        println!("{documented}/{} public function(s) documented ({}%)", public.len(), documented * 100 / public.len());
    }
    if !ws.info.dependencies.is_empty() {
        let deps: Vec<&str> = ws.info.dependencies.iter().map(String::as_str).collect();
        println!("dependencies: {}", deps.join(", "));
    }

    let errors: Vec<(&str, &String)> = ws.files.iter().flat_map(|f| f.parse_errors.iter().map(move |e| (f.path.as_str(), e))).collect();
    if !errors.is_empty() {
        println!("\n{} parse error(s) — results may be partial:", errors.len());
        for (path, e) in errors.iter().take(5) {
            println!("  {path}:{e}");
        }
    }
}

fn files_table(ws: &Workspace) {
    rule("FILES");
    let edges = ws.file_dependencies();
    let mut out_deg: BTreeMap<FileId, usize> = BTreeMap::new();
    let mut in_deg: BTreeMap<FileId, usize> = BTreeMap::new();
    for ((from, to), n) in &edges {
        *out_deg.entry(*from).or_default() += n;
        *in_deg.entry(*to).or_default() += n;
    }

    println!("  {:<26} {:<34} {:>5} {:>7} {:>7}", "MODULE", "FILE", "DEFS", "USES", "USED");
    // Several roots can share the empty module path — a library plus its
    // binaries. Only the first owns the name `crate`.
    let mut root_seen = false;
    for (fid, f) in ws.files.iter().enumerate() {
        let module = if f.module_path.is_empty() {
            if root_seen {
                "crate (bin)".to_string()
            } else {
                root_seen = true;
                "crate".to_string()
            }
        } else {
            format!("crate::{}", f.module_path.join("::"))
        };
        println!("  {:<26} {:<34} {:>5} {:>7} {:>7}", truncate(&module, 26), truncate(&f.path, 34), f.symbols.len(), out_deg.get(&fid).copied().unwrap_or(0), in_deg.get(&fid).copied().unwrap_or(0),);
    }
    println!("\n  USES = references out to other files · USED = references in from them");
}

fn interlinking(ws: &Workspace) {
    rule("FILE INTERLINKING");
    let edges = ws.file_dependencies();
    if edges.is_empty() {
        println!("  (no cross-file references resolved)");
        return;
    }
    let mut rows: Vec<((FileId, FileId), usize)> = edges.into_iter().collect();
    rows.sort_by_key(|((f, t), n)| (std::cmp::Reverse(*n), *f, *t));
    for ((from, to), n) in rows {
        println!("  {:<32} → {:<32} {} ref(s)", truncate(&ws.files[from].path, 32), truncate(&ws.files[to].path, 32), n);
    }
}

// ----------------------------------------------------------------- outline

fn outline(ws: &Workspace, multi: bool, show_docs: bool) {
    rule("OUTLINE");
    type Block = (Option<OwnerKind>, String, Option<String>);

    for f in &ws.files {
        if multi {
            println!("\n  {}", f.path);
        }
        let pad = if multi { "  " } else { "" };

        let mut tree: BTreeMap<String, BTreeMap<Block, Vec<&Symbol>>> = BTreeMap::new();
        for s in &f.symbols {
            if s.kind == SymbolKind::Module {
                continue;
            }
            let block: Block = match &s.owner {
                Some(o) => (s.owner_kind, o.clone(), s.implements.clone()),
                None => (None, String::new(), None),
            };
            tree.entry(s.module_path.join("::")).or_default().entry(block).or_default().push(s);
        }
        if tree.is_empty() {
            println!("{pad}  (no definitions)");
        }

        for (module, blocks) in &tree {
            let indent = if module.is_empty() {
                pad.to_string()
            } else {
                println!("{pad}  mod {module}");
                format!("{pad}  ")
            };
            for ((kind, owner, implements), syms) in blocks {
                let inner = if owner.is_empty() {
                    indent.clone()
                } else {
                    let heading = match (kind, implements) {
                        (Some(OwnerKind::TraitDecl), _) => format!("trait {owner}"),
                        (_, Some(tr)) => format!("impl {tr} for {owner}"),
                        (_, None) => format!("impl {owner}"),
                    };
                    println!("{indent}  {heading}");
                    format!("{indent}  ")
                };
                let mut syms = syms.clone();
                syms.sort_by_key(|s| s.loc.line);
                for s in syms {
                    println!("{inner}  {} {:<8} {}{:<28} L{}", s.visibility.tag(), s.kind.tag(), if s.is_async { "async " } else { "" }, s.name, s.loc.line);
                    if show_docs {
                        if let Some(summary) = s.doc_summary() {
                            println!("{inner}      {}", truncate(&summary, 76));
                        }
                    }
                }
            }
        }
    }
    println!("\n  legend: + pub   ~ pub(crate)/(super)   - private");
}

// ------------------------------------------------------- user-defined calls

fn user_calls(ws: &Workspace) {
    rule("CALLS IN ORDER");
    println!("  Each function's calls in the order they run, with the arguments");
    println!("  passed, what comes back, and where the result is stored.");
    println!("  Standard-library and dependency calls are excluded.\n");
    println!("  Indentation follows the loops and branches the calls sit in.\n");
    println!("  ->  same file      =>  another file in this crate");
    println!("  ~>  another file, matched by unique method name (a bet, not a fact)");
    println!("  ?   several definitions share the name");

    let mut printed_any = false;

    for (fid, f) in ws.files.iter().enumerate() {
        let mut per_file: Vec<String> = Vec::new();
        let mut callables: Vec<&Symbol> = f.callables().collect();
        callables.sort_by_key(|s| s.loc.line);

        for s in callables {
            // Source order is execution order closely enough to read by, and it
            // is the one ordering that answers "what happens first".
            let mut steps: Vec<&Reference> = f.callees_of(s.id).into_iter().filter(|r| r.is_call() && r.to.is_user_code()).collect();
            steps.sort_by_key(|r| (r.loc.line, r.loc.col));

            let mut shown: Vec<String> = Vec::new();
            let mut skipped = 0usize;

            for r in &steps {
                let (marker, target) = match &r.to {
                    Resolution::Local(id) if f.symbols[*id].kind.is_callable() => ("->", Some(GlobalId { file: fid, sym: *id })),
                    Resolution::CrossFile(g) if ws.symbol(*g).kind.is_callable() => (if r.by_name { "~>" } else { "=>" }, Some(*g)),
                    Resolution::Ambiguous(_) => ("? ", None),
                    // A resolved target that is not callable is a type being
                    // constructed, not a call.
                    _ => {
                        skipped += 1;
                        continue;
                    }
                };

                let name = match target {
                    Some(g) => ws.symbol(g).qualified.clone(),
                    None => r.path.clone(),
                };
                let ret = target.and_then(|g| ws.symbol(g).ret.clone()).map(|t| format!(" -> {}", truncate(&t, 34))).unwrap_or_default();
                let call = format!("{}({})", name, r.args.join(", "));
                let stored = match &r.binding {
                    Some(b) => format!("{} = ", truncate(b, 24)),
                    None => String::new(),
                };

                // Indent by nesting, so a call inside a loop reads as inside it.
                let pad = "  ".repeat(r.context.len());
                let note = if r.in_loop() {
                    "  [repeats]"
                } else if r.is_conditional() {
                    "  [conditional]"
                } else {
                    ""
                };
                shown.push(format!("    L{:<5} {} {pad}{}{}{}", r.loc.line, marker, truncate(&format!("{stored}{call}"), 70 - pad.len().min(20)), ret, note));
            }

            if shown.is_empty() {
                continue;
            }
            let mut block = format!("\n  {} (L{})\n", s.qualified, s.loc.line);
            for line in shown {
                block.push_str(&line);
                block.push('\n');
            }
            if skipped > 0 {
                block.push_str(&format!("            \u{b7} {skipped} construction(s) of local types, not shown\n"));
            }
            per_file.push(block);
        }

        if !per_file.is_empty() {
            printed_any = true;
            if ws.files.len() > 1 {
                println!("\n  -- {} --", ws.files[fid].path);
            }
            for b in per_file {
                print!("{b}");
            }
        }
    }

    if !printed_any {
        println!("\n  (no calls between functions defined in this crate)");
    }
}

// --------------------------------------------------------------- callers

fn callers(ws: &Workspace) {
    rule("CALLERS OF EACH DEFINITION");
    println!("  Crate-wide. A caller in another file is marked with its path.\n");

    // target -> caller -> lines
    let mut incoming: BTreeMap<GlobalId, BTreeMap<String, Vec<usize>>> = BTreeMap::new();
    for (fid, f) in ws.files.iter().enumerate() {
        for r in &f.references {
            let Some(target) = r.global_target(fid) else {
                continue;
            };
            let who = match r.from {
                Some(id) => {
                    let caller = &f.symbols[id].qualified;
                    if target.file == fid { caller.clone() } else { format!("{caller}  [{}]", short_path(&f.path)) }
                }
                None => format!("<item level>  [{}]", short_path(&f.path)),
            };
            incoming.entry(target).or_default().entry(who).or_default().push(r.loc.line);
        }
    }

    if incoming.is_empty() {
        println!("  (nothing is called from anywhere the analyzer can see)");
        return;
    }

    let mut last_file = usize::MAX;
    for (target, by_caller) in &incoming {
        if !ws.symbol(*target).kind.is_callable() {
            continue;
        }
        if target.file != last_file && ws.files.len() > 1 {
            println!("\n  ── {} ──", ws.files[target.file].path);
            last_file = target.file;
        }
        let sym = ws.symbol(*target);
        let total: usize = by_caller.values().map(Vec::len).sum();
        println!("\n  {} (L{})  ← {} call site(s)", sym.qualified, sym.loc.line, total);
        for (who, at) in by_caller {
            println!("    ← {:<48} L{}", truncate(who, 48), join_lines(at));
        }
    }
}

// ---------------------------------------------------- entry points / dead

fn entry_points(ws: &Workspace) {
    rule("ENTRY POINTS & UNREFERENCED");

    let mut roots: Vec<GlobalId> = Vec::new();
    let mut orphans: Vec<GlobalId> = Vec::new();
    for g in ws.unreferenced() {
        let s = ws.symbol(g);
        let reachable = s.name == "main" || s.implements.is_some() || s.owner_kind == Some(OwnerKind::TraitDecl) || (s.visibility == Visibility::Public && ws.files.len() == 1);
        if reachable {
            roots.push(g);
        } else {
            orphans.push(g);
        }
    }

    println!("\n  reachable from outside, so not dead:");
    if roots.is_empty() {
        println!("    (none)");
    }
    for g in &roots {
        let s = ws.symbol(*g);
        let why = if s.name == "main" {
            "binary entry point"
        } else if s.owner_kind == Some(OwnerKind::TraitDecl) {
            "trait method — a contract, called through impls"
        } else if s.implements.is_some() {
            "trait impl — called through the trait"
        } else {
            "public API"
        };
        println!("    {:<44} L{:<6} {}", truncate(&ws.full_name(*g), 44), s.loc.line, why);
    }

    let scope = if ws.files.len() > 1 { "this crate" } else { "this file" };
    println!("\n  never called anywhere in {scope} (dead-code candidates):");
    if orphans.is_empty() {
        println!("    (none)");
    }
    for g in &orphans {
        let s = ws.symbol(*g);
        println!("    {:<44} L{:<6} {}", truncate(&ws.full_name(*g), 44), s.loc.line, if s.visibility == Visibility::Public { "pub — may be used by dependents" } else { "" });
    }
}

// ------------------------------------------------------------- library side

fn library_summary(ws: &Workspace, detailed: bool) {
    rule("LIBRARY DEPENDENCIES (std + crates)");
    println!("  Deliberately kept out of the call graph above: you did not write");
    println!("  this code and cannot change it.\n");

    // origin -> name -> count
    let mut by_origin: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for f in &ws.files {
        for r in &f.references {
            if !f.is_interesting(r) {
                continue;
            }
            let Resolution::Unresolved { origin, provenance } = &r.to else {
                continue;
            };
            if *provenance != Provenance::ThirdParty {
                continue;
            }
            let key = origin.clone().unwrap_or_else(|| "std (macro)".into());
            *by_origin.entry(key).or_default().entry(format!("{} {}", r.kind.tag(), r.name)).or_default() += 1;
        }
    }

    if by_origin.is_empty() {
        println!("  (none detected)");
        return;
    }

    let mut rows: Vec<(&String, &BTreeMap<String, usize>)> = by_origin.iter().collect();
    rows.sort_by_key(|(o, m)| (std::cmp::Reverse(m.values().sum::<usize>()), (*o).clone()));

    let limit = if detailed { usize::MAX } else { 12 };
    for (origin, uses) in rows.iter().take(limit) {
        let total: usize = uses.values().sum();
        println!("  {:<44} {} site(s)", truncate(origin, 44), total);
        if detailed {
            let mut items: Vec<(&String, &usize)> = uses.iter().collect();
            items.sort_by_key(|(n, c)| (std::cmp::Reverse(**c), (*n).clone()));
            for (name, count) in items {
                println!("      {:<44} ×{}", name, count);
            }
        }
    }
    if rows.len() > limit {
        println!("  … and {} more origin(s); pass --all for detail", rows.len() - limit);
    }
}

// ------------------------------------------------------------- diagnostics

fn diagnostics(ws: &Workspace) {
    rule("DIAGNOSTICS");

    // Method calls with no inferable receiver type.
    let mut un_inferred: BTreeMap<&str, usize> = BTreeMap::new();
    // First-party-looking names the linker could not place.
    let mut unlinked: BTreeMap<String, usize> = BTreeMap::new();
    let mut ambiguous: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for f in &ws.files {
        for r in &f.references {
            match &r.to {
                Resolution::Unresolved { provenance: Provenance::Unknown, origin } => {
                    if origin.is_none() && r.kind == RefKind::MethodCall {
                        *un_inferred.entry(r.name.as_str()).or_default() += 1;
                    } else if f.is_interesting(r) {
                        *unlinked.entry(origin.clone().unwrap_or_else(|| r.path.clone())).or_default() += 1;
                    }
                }
                Resolution::Unresolved { provenance: Provenance::FirstParty, origin } if f.is_interesting(r) => {
                    *unlinked.entry(origin.clone().unwrap_or_else(|| r.path.clone())).or_default() += 1;
                }
                Resolution::Ambiguous(ids) => {
                    ambiguous.entry(r.name.clone()).or_default().extend(ids.iter().map(|i| f.symbols[*i].qualified.clone()));
                }
                _ => {}
            }
        }
    }

    if !unlinked.is_empty() {
        println!("\n  first-party names that could not be linked to a definition:");
        println!("  (a re-export chain, a macro-generated item, or a file not in the module tree)");
        let mut rows: Vec<(&String, &usize)> = unlinked.iter().collect();
        rows.sort_by_key(|(n, c)| (std::cmp::Reverse(**c), (*n).clone()));
        for (name, count) in rows.iter().take(15) {
            println!("    {:<50} ×{}", truncate(name, 50), count);
        }
        if rows.len() > 15 {
            println!("    … and {} more", rows.len() - 15);
        }
    }

    if !ws.missing_modules.is_empty() {
        println!("\n  `mod` declared with no file found:");
        for (parent, name) in &ws.missing_modules {
            let scope = if parent.is_empty() { "crate".to_string() } else { format!("crate::{}", parent.join("::")) };
            println!("    {scope}::{name}");
        }
    }

    if !ambiguous.is_empty() {
        println!("\n  method names matching several definitions (needs type inference):");
        for (name, cands) in ambiguous.iter().take(15) {
            println!("    .{}()  →  {}", name, cands.iter().cloned().collect::<Vec<_>>().join(" | "));
        }
        if ambiguous.len() > 15 {
            println!("    … and {} more", ambiguous.len() - 15);
        }
    }

    if !un_inferred.is_empty() {
        let total: usize = un_inferred.values().sum();
        println!("\n  {} method call(s) on receivers whose type is not inferred here;", total);
        println!("  {} distinct name(s). These need a type checker, not an index.", un_inferred.len());
    }

    if unlinked.is_empty() && ws.missing_modules.is_empty() && ambiguous.is_empty() && un_inferred.is_empty() {
        println!("  (nothing to report)");
    }
}

// ------------------------------------------------------------------ helpers

fn join_lines(lines: &[usize]) -> String {
    let mut v: Vec<usize> = lines.to_vec();
    v.sort_unstable();
    v.dedup();
    let shown: Vec<String> = v.iter().take(6).map(usize::to_string).collect();
    if v.len() > 6 { format!("{}, +{} more", shown.join(","), v.len() - 6) } else { shown.join(",") }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    // Keep the tail: the distinguishing part of a path is at the end.
    let chars: Vec<char> = s.chars().collect();
    let tail: String = chars[chars.len() - (max - 1)..].iter().collect();
    format!("…{tail}")
}

fn short_path(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Summary counts, for `--stats`.
pub fn print_stats(ws: &Workspace) {
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &ws.files {
        for s in &f.symbols {
            *kinds.entry(s.kind.tag()).or_default() += 1;
        }
    }
    println!("{} — {} file(s)", ws.info.name.clone().unwrap_or_else(|| "(crate)".into()), ws.files.len());
    for (k, n) in &kinds {
        println!("  {k:<8} {n}");
    }

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut distinct_targets: HashSet<GlobalId> = HashSet::new();
    for (fid, f) in ws.files.iter().enumerate() {
        for r in &f.references {
            if !f.is_interesting(r) {
                continue;
            }
            let bucket = match &r.to {
                Resolution::Local(_) => "same file",
                Resolution::CrossFile(_) => "other file",
                Resolution::Ambiguous(_) => "ambiguous",
                Resolution::Unresolved { provenance, .. } => match provenance {
                    Provenance::ThirdParty => "library",
                    _ => "un-inferred",
                },
            };
            *counts.entry(bucket).or_default() += 1;
            if let Some(g) = r.global_target(fid) {
                distinct_targets.insert(g);
            }
        }
    }
    println!("  references:");
    for (k, n) in &counts {
        println!("    {k:<12} {n}");
    }
    let user: usize = counts.get("same file").unwrap_or(&0) + counts.get("other file").unwrap_or(&0) + counts.get("ambiguous").unwrap_or(&0);
    let lib = *counts.get("library").unwrap_or(&0);
    println!("  user-defined {user} vs library {lib}");
}
