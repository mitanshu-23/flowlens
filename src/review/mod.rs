//! Before and after: what the code did at an earlier commit, what it does now.
//!
//! The reader of a review is not assumed to read code. What they want to know
//! is whether each thing the program does still works the way they expect. So
//! the comparison is organised by *feature*, where a feature is an entry
//! point: a function nothing else in the program calls, like `main`, a request
//! handler, or a public API function. For each one the page shows the steps it
//! took before and the steps it takes now, side by side, with what changed
//! marked — including changes buried several calls deep.
//!
//! Features whose steps did not change are listed as working the same. That
//! reassurance is half the point: an edit to one feature should not have
//! touched the others, and this is where the reader sees that it did not.

pub mod compare;
pub mod flow;
pub mod git;
pub mod glossary;
pub mod page;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::model::*;
use crate::workspace::Workspace;
use compare::{CodeLine, Mark, Row};
use flow::{Flow, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Changed,
    Added,
    Removed,
    Same,
}

impl Status {
    pub fn tag(&self) -> &'static str {
        match self {
            Status::Changed => "changed",
            Status::Added => "added",
            Status::Removed => "removed",
            Status::Same => "same",
        }
    }
}

/// One function, compared across the two versions.
#[derive(Debug, Clone)]
pub struct FnReview {
    pub key: String,
    pub name: String,
    pub owner: Option<String>,
    /// What it is for, in words: the doc summary, else the name spelled out.
    pub label: String,
    pub file: String,
    pub line: usize,
    pub status: Status,
    /// Set only when the signature changed.
    pub signature_change: Option<(String, String)>,
    /// The steps, before and now.
    pub rows: Vec<Row>,
    /// The code, before and now.
    pub code: Vec<CodeLine>,
    pub is_test: bool,
    /// Changed functions this one calls, directly or not, nearest first.
    pub changed_below: Vec<String>,
    /// The user functions it called before, and calls now, each listed once.
    pub calls_before: Vec<String>,
    pub calls_now: Vec<String>,
    /// An entry point: nothing else in the program calls it.
    pub is_feature: bool,
    /// The whole doc comment, of which `label` is the first sentence.
    pub doc: Option<String>,
    /// What it takes and gives back: `(name, type)` per parameter.
    pub params: Vec<(String, String)>,
    pub ret: Option<String>,
    /// Glossary entries it uses: types, constants, statics.
    pub uses: Vec<String>,
    /// Its local variables, as they are now (or were, if it was removed).
    pub vars: Vec<flow::Var>,
}

impl FnReview {
    /// Changed, but not in a way the steps show: a constant, an operator, a
    /// condition on a branch that calls nothing of the user's.
    pub fn details_only(&self) -> bool {
        self.status == Status::Changed && self.signature_change.is_none() && !self.rows.iter().any(Row::differs)
    }
}

/// A feature: an entry point and everything it reaches.
#[derive(Debug, Clone)]
pub struct Feature {
    pub key: String,
    pub status: Status,
    /// Functions it reaches that changed, nearest first.
    pub changed_inside: Vec<String>,
}

pub struct Review {
    pub crate_name: String,
    /// What "before" means, e.g. `HEAD (3f2a1c9 · add upload · 2 hours ago)`.
    pub against: String,
    /// False when there was nothing to compare with — no git, or no commits —
    /// and the page shows the project as it is.
    pub has_history: bool,
    pub fns: BTreeMap<String, FnReview>,
    pub features: Vec<Feature>,
    /// Changed functions no feature reaches — a cycle, or code only tests use.
    pub unreached: Vec<String>,
    /// Tests that were added, removed or edited.
    pub tests: Vec<String>,
    /// Every type, constant and static, with what changed about it.
    pub glossary: BTreeMap<String, glossary::Entry>,
}

/// Compare the code at `path` with the same code at git `reference`.
///
/// With no history to compare against — not a git repository, or one with no
/// commits — the code is compared with itself instead: nothing is marked as
/// changed, and the page is a map of the project as it is. A reference that
/// does not exist is still an error; that is a typo, not a project state.
pub fn run(path: &Path, reference: &str) -> Result<Review, String> {
    let now = Code::load(path)?;
    let name = now.name.clone();
    let new = now.side();
    match git::snapshot(path, reference) {
        Ok(snap) => {
            // The path may not have existed back then: everything is new.
            let old = if snap.path.exists() { Code::load(&snap.path).map(|c| c.side()).unwrap_or_else(|_| Side::empty()) } else { Side::empty() };
            Ok(compare_sides(old, new, name, snap.label.clone(), true))
        }
        Err(git::Error::NoHistory(why)) if reference == "HEAD" => {
            let old = Code::load(path)?.side();
            Ok(compare_sides(old, new, name, why, false))
        }
        Err(e) => Err(e.message().to_string()),
    }
}

/// The code under a path: one crate, or every member of a Cargo workspace.
struct Code {
    /// The crate's name, or the workspace folder's.
    name: String,
    /// Each crate with the prefix its functions are keyed under.
    crates: Vec<(String, Workspace)>,
    /// What file paths are shown relative to.
    base: PathBuf,
}

impl Code {
    fn load(path: &Path) -> Result<Code, String> {
        let read = |p: &Path| Workspace::open(p).map_err(|e| format!("cannot read {}: {e}", p.display()));
        let base = if path.is_dir() { path.to_path_buf() } else { path.parent().unwrap_or(Path::new(".")).to_path_buf() };

        let code = match workspace_members(path) {
            Some(members) if !members.is_empty() => {
                let mut crates = Vec::new();
                for dir in members {
                    let ws = read(&dir)?;
                    if ws.files.is_empty() {
                        continue;
                    }
                    let prefix = ws.info.name.clone().unwrap_or_else(|| folder_name(&dir));
                    crates.push((prefix, ws));
                }
                Code { name: folder_name(path), crates, base }
            }
            _ => {
                let ws = read(path)?;
                let name = ws.info.name.clone().unwrap_or_else(|| "this code".to_string());
                Code { name, crates: vec![("crate".to_string(), ws)], base }
            }
        };
        if code.crates.iter().all(|(_, ws)| ws.files.is_empty()) {
            return Err(format!("found no Rust files under {}", path.display()));
        }
        Ok(code)
    }

    fn side(&self) -> Side {
        let mut side = Side::empty();
        for (prefix, ws) in &self.crates {
            side.add(ws, prefix, &self.base);
        }
        side
    }
}

fn folder_name(path: &Path) -> String {
    path.canonicalize().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "this code".to_string())
}

/// The member crates of a Cargo workspace rooted at `dir`, or `None` when
/// `dir` is not a workspace root. A root that is also a package counts as a
/// member. Globs are supported in their common form, `crates/*`.
pub fn workspace_members(dir: &Path) -> Option<Vec<PathBuf>> {
    if !dir.is_dir() {
        return None;
    }
    let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let lines: Vec<&str> = text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).collect();
    if !lines.contains(&"[workspace]") {
        return None;
    }

    // `members = [ ... ]`, possibly over several lines, inside `[workspace]`.
    let mut section = "";
    let mut list = String::new();
    let mut collecting = false;
    for l in &lines {
        if l.starts_with('[') && l.ends_with(']') && !collecting {
            section = l;
            continue;
        }
        if collecting {
            list.push_str(l);
            if l.contains(']') {
                break;
            }
            continue;
        }
        if section == "[workspace]" {
            if let Some((key, value)) = l.split_once('=') {
                if key.trim() == "members" {
                    list.push_str(value);
                    collecting = !value.contains(']');
                    if !collecting {
                        break;
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    if lines.contains(&"[package]") {
        out.push(dir.to_path_buf());
    }
    for entry in list.split(',').map(|e| e.trim().trim_matches(|c| c == '[' || c == ']' || c == ' ').trim_matches('"')) {
        if entry.is_empty() {
            continue;
        }
        match entry.strip_suffix("/*") {
            Some(parent) => {
                let mut found: Vec<PathBuf> = std::fs::read_dir(dir.join(parent)).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("Cargo.toml").is_file()).collect();
                found.sort();
                out.extend(found);
            }
            None => out.push(dir.join(entry)),
        }
    }
    Some(out)
}

/// What one version knows about each function.
struct Side {
    meta: HashMap<String, Meta>,
    flows: HashMap<String, Flow>,
    glossary: BTreeMap<String, glossary::Entry>,
    /// Per function key, the glossary entries it uses.
    uses: HashMap<String, Vec<String>>,
    /// Functions something outside the tests calls or refers to.
    called: HashSet<String>,
}

struct Meta {
    name: String,
    owner: Option<String>,
    label: String,
    file: String,
    line: usize,
    signature: String,
    /// `impl Trait for Type` — called by the language, not by a person.
    trait_impl: bool,
    doc: Option<String>,
    params: Vec<(String, String)>,
    ret: Option<String>,
}

impl Side {
    /// Add one crate's functions, keyed under `prefix`, with file paths shown
    /// relative to `base`.
    fn add(&mut self, ws: &Workspace, prefix: &str, base: &Path) {
        let keys = flow::function_keys(ws, prefix);
        let flows = flow::flows(ws, &keys);
        let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());

        for (g, key) in &keys {
            let f = ws.file(g.file);
            let s = ws.symbol(*g);
            let abs = Path::new(&f.path).canonicalize().unwrap_or_else(|_| PathBuf::from(&f.path));
            let file = abs.strip_prefix(&base).unwrap_or(Path::new(&f.path)).to_string_lossy().into_owned();
            self.meta.insert(
                key.clone(),
                Meta {
                    name: s.name.clone(),
                    owner: s.owner.clone(),
                    label: s.doc_summary().map(|d| short_label(&d)).unwrap_or_else(|| match (s.name.as_str(), prefix) {
                        // `main` says nothing on its own, and a workspace
                        // has one per binary.
                        ("main", "crate") => "Start the program".to_string(),
                        ("main", name) => format!("Start {name}"),
                        _ => spell_out(&s.name),
                    }),
                    file,
                    line: s.loc.line,
                    signature: s.signature.clone(),
                    trait_impl: s.implements.is_some(),
                    doc: s.doc.clone(),
                    params: s.params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
                    ret: s.ret.clone(),
                },
            );
        }

        for (fid, f) in ws.files.iter().enumerate() {
            for r in &f.references {
                let Some(target) = r.global_target(fid).and_then(|g| keys.get(&g)) else {
                    continue;
                };
                let from_test = r.from.and_then(|sym| keys.get(&GlobalId { file: fid, sym })).and_then(|k| flows.get(k)).is_some_and(|fl| fl.is_test);
                if !from_test {
                    self.called.insert(target.clone());
                }
            }
        }
        self.flows.extend(flows);
        let (entries, uses) = glossary::collect(ws, prefix, &base, &keys);
        self.glossary.extend(entries);
        self.uses.extend(uses);
    }

    fn empty() -> Side {
        Side { meta: HashMap::new(), flows: HashMap::new(), glossary: BTreeMap::new(), uses: HashMap::new(), called: HashSet::new() }
    }

    fn is_test(&self, key: &str) -> bool {
        self.flows.get(key).is_some_and(|f| f.is_test)
    }

    /// A feature: nothing calls it, and it is not a test or a trait method
    /// like `fmt` or `drop` that the language calls on its own.
    fn is_entry(&self, key: &str) -> bool {
        self.flows.contains_key(key) && !self.called.contains(key) && !self.is_test(key) && !self.meta.get(key).is_some_and(|m| m.trait_impl)
    }

    /// The user functions a function calls, in the order it calls them.
    fn callees(&self, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(f) = self.flows.get(key) {
            collect_calls(&f.steps, &mut out);
        }
        out
    }
}

fn collect_calls(steps: &[Step], out: &mut Vec<String>) {
    for s in steps {
        match s {
            Step::Call { target, .. } => out.push(target.clone()),
            Step::Block { steps, .. } => collect_calls(steps, out),
            Step::Exit { .. } | Step::Set { .. } => {}
        }
    }
}

/// Compare two versions of one crate, separate from git so it can be
/// tested directly.
pub fn compare(before: Option<&Workspace>, now: &Workspace, against: String) -> Review {
    let side = |ws: &Workspace| {
        let mut s = Side::empty();
        s.add(ws, "crate", &ws.root);
        s
    };
    let old = before.map(side).unwrap_or_else(Side::empty);
    let name = now.info.name.clone().unwrap_or_else(|| "this code".to_string());
    compare_sides(old, side(now), name, against, true)
}

fn compare_sides(old: Side, new: Side, crate_name: String, against: String, has_history: bool) -> Review {
    let mut keys: Vec<&String> = old.flows.keys().chain(new.flows.keys()).collect();
    keys.sort();
    keys.dedup();

    let mut fns: BTreeMap<String, FnReview> = BTreeMap::new();
    for key in keys {
        let (b, a) = (old.flows.get(key), new.flows.get(key));
        let meta = new.meta.get(key).or_else(|| old.meta.get(key)).expect("every flow has metadata");
        let (status, rows, code) = match (b, a) {
            (Some(b), Some(a)) => (if b.fingerprint == a.fingerprint { Status::Same } else { Status::Changed }, compare::diff_steps(&b.steps, &a.steps), compare::diff_lines(&b.source, b.first_line, &a.source, a.first_line)),
            (None, Some(a)) => (Status::Added, compare::one_sided(&a.steps, Mark::Added), compare::diff_lines("", 1, &a.source, a.first_line)),
            (Some(b), None) => (Status::Removed, compare::one_sided(&b.steps, Mark::Removed), compare::diff_lines(&b.source, b.first_line, "", 1)),
            (None, None) => unreachable!("key came from one of the two sides"),
        };
        let signature_change = match (old.meta.get(key), new.meta.get(key)) {
            (Some(o), Some(n)) if o.signature != n.signature => Some((o.signature.clone(), n.signature.clone())),
            _ => None,
        };
        fns.insert(
            key.clone(),
            FnReview {
                key: key.clone(),
                name: meta.name.clone(),
                owner: meta.owner.clone(),
                label: meta.label.clone(),
                file: meta.file.clone(),
                line: meta.line,
                status,
                signature_change,
                rows,
                code,
                is_test: new.is_test(key) || (a.is_none() && old.is_test(key)),
                changed_below: Vec::new(),
                calls_before: dedup(old.callees(key)),
                calls_now: dedup(new.callees(key)),
                is_feature: false,
                doc: meta.doc.clone(),
                params: meta.params.clone(),
                ret: meta.ret.clone(),
                uses: new.uses.get(key).or_else(|| old.uses.get(key)).cloned().unwrap_or_default(),
                vars: a.or(b).map(|f| f.vars.clone()).unwrap_or_default(),
            },
        );
    }

    // What each function reaches, in either version: a call removed by the
    // edit still counts, since that is a change to the feature.
    let reach = |start: &str| -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::from([start.to_string()]);
        let mut order = Vec::new();
        let mut queue = VecDeque::from([start.to_string()]);
        while let Some(k) = queue.pop_front() {
            for c in old.callees(&k).into_iter().chain(new.callees(&k)) {
                if seen.insert(c.clone()) {
                    order.push(c.clone());
                    queue.push_back(c);
                }
            }
        }
        order
    };
    let changed_set: HashSet<String> = fns.values().filter(|f| f.status != Status::Same && !f.is_test).map(|f| f.key.clone()).collect();
    let changed = |k: &String| changed_set.contains(k);

    let mut below: HashMap<String, Vec<String>> = fns.keys().map(|k| (k.clone(), reach(k).into_iter().filter(changed).collect())).collect();
    for (k, f) in fns.iter_mut() {
        f.changed_below = below.remove(k).unwrap_or_default();
    }

    let mut entries: Vec<String> = fns.keys().filter(|k| new.is_entry(k) || (!new.flows.contains_key(*k) && old.is_entry(k))).cloned().collect();
    entries.sort_by_key(|k| (fns[k].file.clone(), fns[k].line));

    for k in &entries {
        if let Some(f) = fns.get_mut(k) {
            f.is_feature = true;
        }
    }

    let mut features = Vec::new();
    let mut covered: HashSet<String> = HashSet::new();
    for key in &entries {
        let reached = reach(key);
        covered.insert(key.clone());
        covered.extend(reached.iter().cloned());
        let changed_inside: Vec<String> = reached.into_iter().filter(changed).collect();
        let own = fns[key.as_str()].status;
        let status = match own {
            Status::Same if !changed_inside.is_empty() => Status::Changed,
            other => other,
        };
        features.push(Feature { key: key.clone(), status, changed_inside });
    }
    features.sort_by_key(|f| f.status);

    let unreached = fns.values().filter(|f| changed(&f.key) && !covered.contains(&f.key)).map(|f| f.key.clone()).collect();
    let tests = fns.values().filter(|f| f.is_test && f.status != Status::Same).map(|f| f.key.clone()).collect();

    Review { crate_name, against, has_history, glossary: glossary::compare(old.glossary, new.glossary), fns, features, unreached, tests }
}

/// Each name once, first occurrence kept.
fn dedup(names: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    names.into_iter().filter(|n| seen.insert(n.clone())).collect()
}

/// Longest label, in characters, before it is cut at a word boundary.
const MAX_LABEL: usize = 64;

/// A doc summary cut to its gist: up to the first colon, dash or aside, and
/// without code quoting. "Load whatever `path` names: a crate directory, …"
/// becomes "Load whatever path names".
fn short_label(doc: &str) -> String {
    let mut s = doc.replace('`', "");
    for stop in [": ", " — ", " - ", " (", "; "] {
        if let Some(i) = s.find(stop) {
            if i >= 12 {
                s.truncate(i);
            }
        }
    }
    let s = s.trim().trim_end_matches('.').to_string();
    if s.chars().count() <= MAX_LABEL {
        return s;
    }
    let cut: String = s.chars().take(MAX_LABEL).collect();
    let cut = cut.rsplit_once(' ').map(|(head, _)| head).unwrap_or(&cut);
    format!("{}\u{2026}", cut.trim_end_matches([',', ';']))
}

/// `collect_symbols` -> `Collect symbols`, for functions with no doc comment.
fn spell_out(name: &str) -> String {
    let words = name.trim_start_matches('_').replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => name.to_string(),
    }
}
