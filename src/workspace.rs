//! A whole crate: one [`FileIndex`] per module file, plus the pass that turns
//! each file's unresolved names into real edges between files.
//!
//! Indexing is per-file and independent, so a name defined elsewhere starts out
//! `Unresolved` with the `use` it came in through. This module then walks those
//! imports to the file that actually defines the name and upgrades the
//! reference to [`Resolution::CrossFile`]. What is left unresolved after that is
//! either a dependency or a method call needing type inference — never a
//! first-party call the analyzer merely failed to look up.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::index::{FileIndex, index_file_in};
use crate::model::*;
use crate::modmap::{self, ModuleMap};
use crate::provenance::{CrateInfo, Provenance};

/// How many `pub use` hops to follow before giving up. Facade modules chain a
/// couple of levels; anything deeper is more likely a cycle than a re-export.
const MAX_REEXPORT_HOPS: usize = 4;

/// Method names so common in `std` that a same-named method in this crate is
/// weak evidence. Resolving `vec.len()` to your own `len` would be wrong more
/// often than right, so these are never matched by name alone.
const ABMIGUOUS_WITH_STD: &[&str] = &[
    "new",
    "len",
    "is_empty",
    "get",
    "insert",
    "remove",
    "push",
    "pop",
    "clear",
    "iter",
    "next",
    "clone",
    "to_string",
    "from",
    "into",
    "as_str",
    "as_ref",
    "unwrap",
    "expect",
    "map",
    "filter",
    "collect",
    "contains",
    "extend",
    "default",
    "fmt",
    "eq",
    "cmp",
    "hash",
    "drop",
    "add",
    "sub",
    "read",
    "write",
];

pub struct Workspace {
    pub root: PathBuf,
    pub info: CrateInfo,
    pub files: Vec<FileIndex>,
    /// Module path -> the file that defines it.
    by_module: HashMap<Vec<String>, FileId>,
    /// Per file: qualified name -> symbol.
    by_qualified: Vec<HashMap<String, SymbolId>>,
    /// Per file: bare name -> every symbol with that name.
    by_name: Vec<HashMap<String, Vec<SymbolId>>>,
    /// Crate-wide qualified name -> every symbol with it. An inherent impl can
    /// sit in a different file from the type it is on, so the module path in a
    /// `use` does not always lead to the definition.
    global_qualified: HashMap<String, Vec<GlobalId>>,
    /// `mod` declarations with no file behind them.
    pub missing_modules: Vec<(Vec<String>, String)>,
    /// Cross-file edges closed by `Workspace::link`.
    pub linked: usize,
    /// Of those, how many rest on a unique-name match rather than an import.
    pub linked_by_name: usize,
}

impl Workspace {
    /// Index every file of the crate containing `start`, then link them.
    pub fn load(start: &Path) -> std::io::Result<Workspace> {
        let map: ModuleMap = modmap::discover(start);
        let mut info = CrateInfo::discover(start);

        // Top-level module names let `db::connect` be recognised as
        // first-party even without a `crate::` prefix.
        for f in &map.files {
            if f.module_path.len() == 1 {
                info.modules.insert(f.module_path[0].clone());
            }
        }

        let root = if start.is_dir() { start.to_path_buf() } else { start.parent().unwrap_or(Path::new(".")).to_path_buf() };

        let mut files = Vec::new();
        let mut by_module = HashMap::new();
        for mf in &map.files {
            let src = std::fs::read_to_string(&mf.path)?;
            let display = mf.path.to_string_lossy().to_string();
            // Several roots can claim the empty module path (a lib plus its
            // binaries). The first — the library — keeps it.
            by_module.entry(mf.module_path.clone()).or_insert(files.len());
            files.push(index_file_in(&display, &src, &info, mf.module_path.clone()));
        }

        let by_qualified = files.iter().map(|f| f.symbols.iter().map(|s| (s.qualified.clone(), s.id)).collect()).collect();
        let by_name = files
            .iter()
            .map(|f| {
                let mut m: HashMap<String, Vec<SymbolId>> = HashMap::new();
                for s in &f.symbols {
                    m.entry(s.name.clone()).or_default().push(s.id);
                }
                m
            })
            .collect();

        let mut global_qualified: HashMap<String, Vec<GlobalId>> = HashMap::new();
        for (fid, f) in files.iter().enumerate() {
            for s in &f.symbols {
                global_qualified.entry(s.qualified.clone()).or_default().push(GlobalId { file: fid, sym: s.id });
            }
        }

        let mut ws = Workspace { root, info, files, by_module, by_qualified, by_name, global_qualified, missing_modules: map.missing, linked: 0, linked_by_name: 0 };
        ws.link();
        Ok(ws)
    }

    /// Load whatever `path` names: a crate directory, a file inside a crate
    /// (which still gets the whole crate, so cross-file calls resolve), or a
    /// lone file indexed by itself.
    pub fn open(path: &Path) -> std::io::Result<Workspace> {
        if path.is_dir() {
            return Workspace::load(path);
        }
        // Try the crate first: if this file is part of the module tree,
        // indexing the crate gives it cross-file resolution too.
        if let Ok(ws) = Workspace::load(path) {
            let target = path.canonicalize().ok();
            let in_tree = ws.files.iter().any(|f| Path::new(&f.path).canonicalize().ok() == target);
            if in_tree {
                return Ok(ws);
            }
        }
        // Not wired into any crate — index it alone, but still classify
        // against whatever Cargo.toml is above it.
        let src = std::fs::read_to_string(path)?;
        let info = CrateInfo::discover(path);
        Ok(Workspace::single_file(&path.to_string_lossy(), &src, info))
    }

    /// Index a single file with no crate context. Cross-file linking cannot
    /// run, so first-party names stay unresolved.
    pub fn single_file(path: &str, src: &str, info: CrateInfo) -> Workspace {
        let index = index_file_in(path, src, &info, Vec::new());
        let by_qualified = vec![index.symbols.iter().map(|s| (s.qualified.clone(), s.id)).collect()];
        let mut names: HashMap<String, Vec<SymbolId>> = HashMap::new();
        for s in &index.symbols {
            names.entry(s.name.clone()).or_default().push(s.id);
        }
        Workspace {
            root: PathBuf::from("."),
            info,
            files: vec![index],
            by_module: HashMap::from([(Vec::new(), 0)]),
            by_qualified,
            by_name: vec![names],
            global_qualified: HashMap::new(),
            missing_modules: Vec::new(),
            linked: 0,
            linked_by_name: 0,
        }
    }

    pub fn file(&self, id: FileId) -> &FileIndex {
        &self.files[id]
    }

    pub fn symbol(&self, g: GlobalId) -> &Symbol {
        &self.files[g.file].symbols[g.sym]
    }

    /// Fully qualified across the crate: `crate::db::Pool::new`.
    pub fn full_name(&self, g: GlobalId) -> String {
        let f = &self.files[g.file];
        let sym = &f.symbols[g.sym];
        let mut parts = vec!["crate".to_string()];
        parts.extend(f.module_path.iter().cloned());
        parts.push(sym.qualified.clone());
        parts.join("::")
    }

    // ------------------------------------------------------------ linking

    /// Close every cross-file edge that can be closed.
    fn link(&mut self) {
        // Resolve into a side buffer: resolution reads other files' indexes
        // while this file's references are being rewritten.
        let mut updates: Vec<(FileId, usize, GlobalId)> = Vec::new();

        for (fid, f) in self.files.iter().enumerate() {
            for (ri, r) in f.references.iter().enumerate() {
                let Resolution::Unresolved { origin, provenance } = &r.to else {
                    continue;
                };
                // A dependency is not ours to chase.
                if *provenance == Provenance::ThirdParty {
                    continue;
                }
                if let Some(g) = self.resolve_across(fid, r, origin.as_deref(), 0) {
                    updates.push((fid, ri, g));
                }
            }
        }

        self.linked = updates.len();
        for (fid, ri, g) in updates {
            self.files[fid].references[ri].to = Resolution::CrossFile(g);
        }

        // Anything still unresolved that traces out to a dependency is
        // reclassified, so it lands in the library summary rather than being
        // reported as a first-party name the analyzer failed to find.
        let mut reclassified: Vec<(FileId, usize)> = Vec::new();
        for (fid, f) in self.files.iter().enumerate() {
            for (ri, r) in f.references.iter().enumerate() {
                let Resolution::Unresolved { origin, provenance } = &r.to else {
                    continue;
                };
                if *provenance == Provenance::ThirdParty {
                    continue;
                }
                if self.traces_to_dependency(fid, r, origin.as_deref()) {
                    reclassified.push((fid, ri));
                }
            }
        }
        for (fid, ri) in reclassified {
            if let Resolution::Unresolved { provenance, .. } = &mut self.files[fid].references[ri].to {
                *provenance = Provenance::ThirdParty;
            }
        }

        self.link_methods_by_name();
    }

    /// Second pass, for method calls no import can explain.
    ///
    /// `lines.line_col(..)` gives nothing to look up: the receiver's type is
    /// not inferred, and the method may well live in another file. But if
    /// exactly one method in the whole crate carries that name, it is almost
    /// certainly the one meant. More than one, and the guess is dropped rather
    /// than made — an honest gap beats a wrong edge.
    fn link_methods_by_name(&mut self) {
        let mut crate_methods: HashMap<&str, Vec<GlobalId>> = HashMap::new();
        for (fid, f) in self.files.iter().enumerate() {
            for s in &f.symbols {
                if s.kind == SymbolKind::Method {
                    crate_methods.entry(s.name.as_str()).or_default().push(GlobalId { file: fid, sym: s.id });
                }
            }
        }

        let mut updates: Vec<(FileId, usize, GlobalId)> = Vec::new();
        for (fid, f) in self.files.iter().enumerate() {
            for (ri, r) in f.references.iter().enumerate() {
                if r.kind != RefKind::MethodCall {
                    continue;
                }
                if !matches!(r.to, Resolution::Unresolved { .. }) {
                    continue;
                }
                if ABMIGUOUS_WITH_STD.contains(&r.name.as_str()) {
                    continue;
                }
                // Distinct definitions only: a method declared on a trait and
                // implemented once is the same method, not two candidates.
                match crate_methods.get(r.name.as_str()).map(Vec::as_slice) {
                    Some([only]) if only.file != fid => updates.push((fid, ri, *only)),
                    _ => {}
                }
            }
        }

        self.linked += updates.len();
        self.linked_by_name = updates.len();
        for (fid, ri, g) in updates {
            self.files[fid].references[ri].to = Resolution::CrossFile(g);
            self.files[fid].references[ri].by_name = true;
        }
    }

    /// Find the definition a reference points at, in some other file.
    fn resolve_across(&self, from: FileId, r: &Reference, origin: Option<&str>, hop: usize) -> Option<GlobalId> {
        if hop > MAX_REEXPORT_HOPS {
            return None;
        }

        let candidates = self.candidate_paths(from, r, origin);
        for abs in &candidates {
            if let Some(found) = self.lookup_absolute(abs, from, r, hop) {
                return Some(found);
            }
        }

        // Last resort: the name is qualified and defined exactly once in the
        // crate, just not under the module the import pointed at. `SourceFile`
        // living in one file with `impl SourceFile` in another is the usual
        // reason.
        for abs in &candidates {
            for split in (1..abs.len()).rev() {
                let tail = abs[split..].join("::");
                if let Some([only]) = self.global_qualified.get(&tail).map(Vec::as_slice) {
                    if only.file != from {
                        return Some(*only);
                    }
                }
            }
        }
        None
    }

    /// Does this name's import chain lead out of the crate?
    ///
    /// A root that re-exports a dependency — `pub use parser::SyntaxKind` —
    /// makes `crate::SyntaxKind` look first-party when it is not. Following the
    /// chain one step tells the difference, which matters because the whole
    /// point of the split is to keep dependency code out of the user's graph.
    fn traces_to_dependency(&self, from: FileId, r: &Reference, origin: Option<&str>) -> bool {
        let here = &self.files[from].module_path;
        let Some(origin) = origin else { return false };
        let Some(abs) = self.normalize(origin, here) else {
            return false;
        };
        for split in (0..abs.len()).rev() {
            let (module, rest) = abs.split_at(split);
            let Some(&target) = self.by_module.get(module) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            if let Some(next) = self.files[target].imports.get(&rest[0]) {
                if self.info.classify(Some(next), next, false) == Provenance::ThirdParty {
                    return true;
                }
                // One more hop, for a facade over a facade.
                if let Some(deeper) = self.normalize(next, &self.files[target].module_path) {
                    if let Some(&t2) = self.by_module.get(deeper.as_slice()) {
                        if let Some(n2) = deeper.last().and_then(|leaf| self.files[t2].imports.get(leaf)) {
                            if self.info.classify(Some(n2), n2, false) == Provenance::ThirdParty {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        let _ = r;
        false
    }

    /// Absolute crate-relative paths worth trying, best guess first.
    fn candidate_paths(&self, from: FileId, r: &Reference, origin: Option<&str>) -> Vec<Vec<String>> {
        let here = &self.files[from].module_path;
        let mut out: Vec<Vec<String>> = Vec::new();

        // Segments of the call-site path after its head, e.g. `Pool::new` -> ["new"].
        let call_segs: Vec<&str> = r.path.split("::").collect();
        let tail: Vec<String> = call_segs.iter().skip(1).map(|s| s.to_string()).collect();

        if let Some(origin) = origin {
            if let Some(base) = self.normalize(origin, here) {
                // `use crate::db::Pool` + `Pool::new()` -> crate::db::Pool::new
                let mut with_tail = base.clone();
                with_tail.extend(tail.iter().cloned());
                if !tail.is_empty() {
                    out.push(with_tail);
                }
                out.push(base);
            }
        }

        // No import, or the import led nowhere: try the written path itself,
        // both as an absolute crate path and relative to this module.
        if call_segs.len() > 1 && !r.path.contains('.') {
            if let Some(abs) = self.normalize(&r.path, here) {
                out.push(abs);
            }
            let mut relative = here.clone();
            relative.extend(call_segs.iter().map(|s| s.to_string()));
            out.push(relative);
        }

        out.dedup();
        out
    }

    /// Turn a written path into crate-relative segments, resolving `crate`,
    /// `self`, `super` and the crate's own name. Returns `None` for a path
    /// rooted in another crate.
    fn normalize(&self, path: &str, here: &[String]) -> Option<Vec<String>> {
        let mut segs: Vec<String> = path.split("::").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if segs.is_empty() {
            return None;
        }

        let head = segs[0].clone();
        if head == "crate" {
            segs.remove(0);
            return Some(segs);
        }
        if self.info.name.as_deref() == Some(head.replace('-', "_").as_str()) {
            segs.remove(0);
            return Some(segs);
        }
        if head == "self" {
            segs.remove(0);
            let mut abs = here.to_vec();
            abs.extend(segs);
            return Some(abs);
        }
        if head == "super" {
            let mut depth = 0;
            while segs.first().map(String::as_str) == Some("super") {
                segs.remove(0);
                depth += 1;
            }
            if depth > here.len() {
                return None;
            }
            let mut abs = here[..here.len() - depth].to_vec();
            abs.extend(segs);
            return Some(abs);
        }
        // A bare head naming one of this crate's modules is crate-relative.
        if self.info.modules.contains(&head) {
            return Some(segs);
        }
        None
    }

    /// Split `abs` into the longest module prefix that is a real file, then
    /// look the remainder up inside that file.
    fn lookup_absolute(&self, abs: &[String], from: FileId, r: &Reference, hop: usize) -> Option<GlobalId> {
        // Longest prefix first: `db::Pool::new` should try module `db::Pool`
        // before module `db`, in case `Pool` is itself a module.
        for split in (0..abs.len()).rev() {
            let (module, rest) = abs.split_at(split);
            let Some(&target) = self.by_module.get(module) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            if target == from {
                // Same file — the local pass already had its chance.
                continue;
            }
            if let Some(sym) = self.lookup_in_file(target, rest) {
                return Some(GlobalId { file: target, sym });
            }
            // Not defined there. It may be re-exported: follow that file's
            // own import of the name.
            if let Some(next) = self.files[target].imports.get(&rest[0]) {
                let next = next.clone();
                if let Some(found) = self.resolve_across(target, r, Some(&next), hop + 1) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Look up `segs` as a qualified name inside one file.
    fn lookup_in_file(&self, file: FileId, segs: &[String]) -> Option<SymbolId> {
        let qualified = segs.join("::");
        if let Some(&id) = self.by_qualified[file].get(&qualified) {
            return Some(id);
        }
        // `Pool::new` where the impl is written on an aliased or generic self
        // type: fall back to the bare name, but only when unambiguous.
        let last = segs.last()?;
        match self.by_name[file].get(last).map(Vec::as_slice) {
            Some([only]) => Some(*only),
            _ => None,
        }
    }

    // ------------------------------------------------------------- queries

    /// Every reference in the crate that lands on `target`.
    pub fn callers_of(&self, target: GlobalId) -> Vec<(FileId, &Reference)> {
        let mut out = Vec::new();
        for (fid, f) in self.files.iter().enumerate() {
            for r in &f.references {
                if r.global_target(fid) == Some(target) {
                    out.push((fid, r));
                }
            }
        }
        out
    }

    /// File-to-file edge weights: how many user-code references cross from one
    /// file into another. This is the interlinking view.
    pub fn file_dependencies(&self) -> BTreeMap<(FileId, FileId), usize> {
        let mut edges: BTreeMap<(FileId, FileId), usize> = BTreeMap::new();
        for (fid, f) in self.files.iter().enumerate() {
            for r in &f.references {
                if let Resolution::CrossFile(g) = &r.to {
                    if g.file != fid {
                        *edges.entry((fid, g.file)).or_default() += 1;
                    }
                }
            }
        }
        edges
    }

}
