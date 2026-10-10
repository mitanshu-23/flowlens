//! Behavioural tests pinning the resolution rules that are easy to get subtly
//! wrong, and the classification that keeps the user's own code separate from
//! the standard library and dependencies.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use flowlens::index::{FileIndex, index_file, index_file_in};
use flowlens::model::*;
use flowlens::provenance::{CrateInfo, Provenance};
use flowlens::workspace::Workspace;

// ------------------------------------------------------------ single file

const SRC: &str = r#"
use std::collections::HashMap;
use serde_json::Value;
use crate::db::connect;

pub struct Repo { pool: u32 }
pub struct Cache { map: HashMap<String, String> }

trait Store { fn get(&self, k: &str) -> Option<String>; }

impl Repo {
    pub fn new() -> Self { Repo { pool: connect() } }
    pub fn load(&self, id: u32) -> Option<String> { self.fetch(id) }
    fn fetch(&self, _id: u32) -> Option<String> { None }
    fn describe(&self) -> String { format!("repo {}", label(self.pool)) }
}

impl Store for Repo {
    fn get(&self, k: &str) -> Option<String> { self.load(k.len() as u32) }
}

impl Cache {
    fn get(&self, k: &str) -> Option<String> { self.map.get(k).cloned() }
}

fn label(n: u32) -> String { n.to_string() }
fn countdown(n: u32) -> u32 { if n == 0 { 0 } else { countdown(n - 1) } }
fn never_used() -> bool { true }
fn as_json(_v: Value) {}

mod util { pub fn helper() -> u8 { 7 } }

fn main() {
    let r = Repo::new();
    r.load(1);
    countdown(3);
    util::helper();
    println!("{}", label(1));
}
"#;

const MANIFEST: &str = r#"
[package]
name = "fixture-crate"
version = "0.1.0"

[dependencies]
serde_json = "1"
ra-ap-syntax = "0.0.293"

[dev-dependencies]
tempfile = "3"
"#;

fn ix() -> FileIndex {
    index_file_in("fixture.rs", SRC, &CrateInfo::from_manifest(MANIFEST), Vec::new())
}

fn find(ix: &FileIndex, qualified: &str) -> SymbolId {
    ix.symbols.iter().find(|s| s.qualified == qualified).unwrap_or_else(|| panic!("no symbol {qualified}; have: {:?}", ix.symbols.iter().map(|s| &s.qualified).collect::<Vec<_>>())).id
}

fn refs_named<'a>(ix: &'a FileIndex, name: &str) -> Vec<&'a Reference> {
    ix.references.iter().filter(|r| r.name == name).collect()
}

#[test]
fn parses_without_errors() {
    assert!(ix().parse_errors.is_empty());
}

#[test]
fn separates_inherent_impl_from_trait_impl() {
    let ix = ix();
    let load = &ix.symbols[find(&ix, "Repo::load")];
    assert_eq!(load.implements, None, "load is on the inherent impl");
    assert_eq!(load.owner_kind, Some(OwnerKind::Impl));

    let get = ix.symbols.iter().find(|s| s.qualified == "Repo::get" && s.implements.is_some()).expect("the trait impl's get");
    assert_eq!(get.implements.as_deref(), Some("Store"));
}

#[test]
fn trait_declaration_is_not_an_impl() {
    let ix = ix();
    assert_eq!(ix.symbols[find(&ix, "Store::get")].owner_kind, Some(OwnerKind::TraitDecl));
}

#[test]
fn bare_self_receiver_resolves_to_own_type() {
    let ix = ix();
    let load = find(&ix, "Repo::load");
    let fetch = find(&ix, "Repo::fetch");
    assert!(ix.callees_of(load).iter().any(|r| r.target() == Some(fetch)), "self.fetch() should bind to Repo::fetch");
}

#[test]
fn field_receiver_does_not_resolve_to_own_type() {
    let ix = ix();
    let cache_get = find(&ix, "Cache::get");
    let call = ix.callees_of(cache_get).into_iter().find(|r| r.name == "get" && r.kind == RefKind::MethodCall).expect("the .get() call site");
    assert_ne!(call.target(), Some(cache_get), "self.map.get() must not be mistaken for Cache::get");
}

#[test]
fn ambiguous_method_names_are_reported_not_guessed() {
    let ix = ix();
    let amb: Vec<&Reference> = ix.references.iter().filter(|r| matches!(r.to, Resolution::Ambiguous(_))).collect();
    assert!(!amb.is_empty(), "the `get` methods should collide");
    for r in amb {
        if let Resolution::Ambiguous(ids) = &r.to {
            assert!(ids.len() > 1);
        }
    }
}

#[test]
fn recursion_shows_up_as_a_self_edge() {
    let ix = ix();
    let cd = find(&ix, "countdown");
    assert!(ix.callees_of(cd).iter().any(|r| r.target() == Some(cd)));
    assert!(ix.callers_of(cd).iter().any(|r| r.from == Some(cd)));
}

#[test]
fn module_qualified_call_resolves() {
    let ix = ix();
    let helper = find(&ix, "util::helper");
    let main = find(&ix, "main");
    assert!(ix.callees_of(main).iter().any(|r| r.target() == Some(helper)));
}

#[test]
fn associated_function_call_resolves() {
    let ix = ix();
    let new = find(&ix, "Repo::new");
    let main = find(&ix, "main");
    assert!(ix.callees_of(main).iter().any(|r| r.target() == Some(new)));
}

#[test]
fn imports_supply_the_origin_of_external_names() {
    let ix = ix();
    let connect = refs_named(&ix, "connect").into_iter().next().expect("the connect() call");
    match &connect.to {
        Resolution::Unresolved { origin, .. } => {
            assert_eq!(origin.as_deref(), Some("crate::db::connect"))
        }
        other => panic!("expected unresolved, got {other:?}"),
    }
}

#[test]
fn method_calls_get_no_invented_origin() {
    let ix = ix();
    let cloned = refs_named(&ix, "cloned").into_iter().next().expect("the .cloned() call");
    assert_eq!(cloned.to, Resolution::Unresolved { origin: None, provenance: Provenance::Unknown });
}

#[test]
fn unreferenced_private_function_is_visible_as_such() {
    let ix = ix();
    let unused = find(&ix, "never_used");
    assert!(ix.callers_of(unused).is_empty());
    assert_eq!(ix.symbols[unused].visibility, Visibility::Private);
}

#[test]
fn line_numbers_are_one_based_and_correct() {
    let src = "a\nbb\nccc\n";
    let lines = LineIndex::new(src);
    assert_eq!(lines.line_col(src, 0), (1, 1));
    assert_eq!(lines.line_col(src, 2), (2, 1));
    assert_eq!(lines.line_col(src, 3), (2, 2));
    assert_eq!(lines.line_col(src, 5), (3, 1));
}

// ------------------------------------------------------- macro recovery

#[test]
fn calls_inside_macro_arguments_are_found() {
    // `format!("repo {}", label(..))` hides the call to `label` inside an
    // unparsed token tree. Without re-parsing it, `label` looks dead.
    let ix = ix();
    let label = find(&ix, "label");
    let describe = find(&ix, "Repo::describe");
    let callers = ix.callers_of(label);
    assert!(callers.iter().any(|r| r.from == Some(describe)), "label() is called from inside format!() in Repo::describe");
    assert!(callers.iter().any(|r| r.via_macro), "and should be marked as recovered from a macro");
}

#[test]
fn macro_recovered_calls_report_the_original_line() {
    let ix = ix();
    let label = find(&ix, "label");
    let src_lines: Vec<&str> = SRC.lines().collect();
    for r in ix.callers_of(label) {
        let line = src_lines[r.loc.line - 1];
        assert!(line.contains("label("), "reference at L{} maps to the wrong line: {line:?}", r.loc.line);
    }
}

#[test]
fn macro_call_itself_is_still_recorded() {
    let ix = ix();
    let println_refs = refs_named(&ix, "println");
    assert!(!println_refs.is_empty());
    assert_eq!(println_refs[0].kind, RefKind::MacroCall);
}

// --------------------------------------------------------- provenance

#[test]
fn std_paths_are_library() {
    let ix = ix();
    let hashmap = refs_named(&ix, "HashMap");
    assert!(!hashmap.is_empty());
    assert!(hashmap.iter().all(|r| r.provenance() == Provenance::ThirdParty));
}

#[test]
fn declared_dependencies_are_library() {
    let ix = ix();
    let value = refs_named(&ix, "Value").into_iter().next().expect("the serde_json::Value type reference");
    assert_eq!(value.provenance(), Provenance::ThirdParty);
}

#[test]
fn crate_paths_are_first_party() {
    let ix = ix();
    let connect = refs_named(&ix, "connect")[0];
    assert_eq!(connect.provenance(), Provenance::FirstParty);
}

#[test]
fn std_macros_are_not_attributed_to_the_user() {
    let ix = ix();
    let p = refs_named(&ix, "println")[0];
    assert_eq!(p.provenance(), Provenance::ThirdParty);
    assert!(!p.provenance().is_user_code());
}

#[test]
fn local_definitions_are_local() {
    let ix = ix();
    let cd = refs_named(&ix, "countdown");
    assert!(cd.iter().any(|r| r.provenance() == Provenance::Local));
}

#[test]
fn manifest_reader_finds_all_dependency_tables() {
    let info = CrateInfo::from_manifest(MANIFEST);
    assert_eq!(info.name.as_deref(), Some("fixture_crate"));
    // Hyphens become underscores, since that is how paths spell them.
    assert!(info.dependencies.contains("ra_ap_syntax"));
    assert!(info.dependencies.contains("serde_json"));
    assert!(info.dependencies.contains("tempfile"));
}

#[test]
fn manifest_reader_handles_table_form_dependencies() {
    let info = CrateInfo::from_manifest("[package]\nname = \"x\"\n\n[dependencies.tokio]\nversion = \"1\"\nfeatures = [\"full\"]\n");
    assert!(info.dependencies.contains("tokio"));
    // `version` and `features` are keys of the dependency, not dependencies.
    assert!(!info.dependencies.contains("version"));
    assert!(!info.dependencies.contains("features"));
}

#[test]
fn no_crate_context_means_unknown_not_library() {
    // Without a manifest, a dependency name cannot be recognised — and must
    // not be silently claimed as the user's own code either.
    let ix = index_file("bare.rs", "use serde_json::Value;\nfn f(_v: Value) {}\n");
    let v = ix.references.iter().find(|r| r.name == "Value").expect("the Value type reference");
    assert_eq!(v.provenance(), Provenance::Unknown);
}

// ------------------------------------------------------------ multi-file

/// A throwaway crate on disk, removed when the test ends.
struct TempCrate {
    dir: PathBuf,
}

impl TempCrate {
    fn new(files: &[(&str, &str)]) -> TempCrate {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("flowlens-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&dir);
        for (rel, contents) in files {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }
        TempCrate { dir }
    }

    fn load(&self) -> Workspace {
        Workspace::load(&self.dir).expect("workspace should load")
    }
}

impl Drop for TempCrate {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn small_crate() -> TempCrate {
    TempCrate::new(&[
        ("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n"),
        ("src/lib.rs", "pub mod db;\npub mod util;\npub mod facade;\n\nuse std::collections::HashMap;\nuse crate::db::connect;\n\npub fn boot() -> u32 { connect() + util::helper() }\npub fn table() -> HashMap<u32, u32> { HashMap::new() }\n"),
        ("src/db.rs", "pub fn connect() -> u32 { 1 }\npub struct Pool;\nimpl Pool { pub fn acquire(&self) -> u32 { 2 } }\n"),
        ("src/util/mod.rs", "pub mod inner;\npub fn helper() -> u32 { inner::deep() }\n"),
        ("src/util/inner.rs", "pub fn deep() -> u32 { 3 }\n"),
        ("src/facade.rs", "pub use crate::db::connect;\n"),
        ("src/main.rs", "use demo::facade::connect;\nfn main() { let _ = connect(); }\n"),
    ])
}

fn gid(ws: &Workspace, file_suffix: &str, qualified: &str) -> GlobalId {
    let (fid, f) = ws.files.iter().enumerate().find(|(_, f)| f.path.replace('\\', "/").ends_with(file_suffix)).unwrap_or_else(|| panic!("no file ending {file_suffix}; have {:?}", ws.files.iter().map(|f| &f.path).collect::<Vec<_>>()));
    let sym = f.symbols.iter().find(|s| s.qualified == qualified).unwrap_or_else(|| panic!("no {qualified} in {file_suffix}")).id;
    GlobalId { file: fid, sym }
}

#[test]
fn module_tree_discovers_nested_files() {
    let tc = small_crate();
    let ws = tc.load();
    let mut found: Vec<String> = ws.files.iter().map(|f| f.path.replace('\\', "/").rsplit('/').next().unwrap().to_string()).collect();
    found.sort();
    assert_eq!(found, vec!["db.rs", "facade.rs", "inner.rs", "lib.rs", "main.rs", "mod.rs"]);
}

#[test]
fn nested_module_path_is_recorded() {
    let tc = small_crate();
    let ws = tc.load();
    let inner = ws.files.iter().find(|f| f.path.ends_with("inner.rs")).unwrap();
    assert_eq!(inner.module_path, vec!["util", "inner"]);
}

#[test]
fn cross_file_call_resolves_through_use() {
    let tc = small_crate();
    let ws = tc.load();
    let connect = gid(&ws, "src/db.rs", "connect");
    let callers = ws.callers_of(connect);
    assert!(callers.iter().any(|(fid, _)| *fid != connect.file), "boot() in lib.rs calls db::connect; callers were {:?}", callers.len());
}

#[test]
fn cross_file_call_through_module_path_resolves() {
    let tc = small_crate();
    let ws = tc.load();
    // `util::helper()` in lib.rs, with no `use` for it.
    let helper = gid(&ws, "src/util/mod.rs", "helper");
    assert!(!ws.callers_of(helper).is_empty());
}

#[test]
fn call_between_two_nested_modules_resolves() {
    let tc = small_crate();
    let ws = tc.load();
    // `inner::deep()` from util/mod.rs, relative to the current module.
    let deep = gid(&ws, "src/util/inner.rs", "deep");
    assert!(!ws.callers_of(deep).is_empty(), "util::helper calls inner::deep");
}

#[test]
fn reexport_is_followed_to_the_real_definition() {
    let tc = small_crate();
    let ws = tc.load();
    // main.rs imports through `facade`, which only re-exports db::connect.
    let connect = gid(&ws, "src/db.rs", "connect");
    let from_main = ws.callers_of(connect).into_iter().any(|(fid, _)| ws.files[fid].path.ends_with("main.rs"));
    assert!(from_main, "the `pub use` in facade.rs should be followed");
}

#[test]
fn file_dependencies_show_the_edges() {
    let tc = small_crate();
    let ws = tc.load();
    let edges = ws.file_dependencies();
    assert!(!edges.is_empty());
    for ((from, to), n) in &edges {
        assert_ne!(from, to, "a file cannot depend on itself");
        assert!(*n > 0);
    }
}

#[test]
fn unique_method_name_links_across_files() {
    let tc = TempCrate::new(&[
        ("Cargo.toml", "[package]\nname = \"m\"\nversion = \"0.1.0\"\n"),
        ("src/lib.rs", "pub mod thing;\nuse crate::thing::Thing;\npub fn go(t: &Thing) -> u32 { t.compute_total() }\n"),
        ("src/thing.rs", "pub struct Thing;\nimpl Thing { pub fn compute_total(&self) -> u32 { 1 } }\n"),
    ]);
    let ws = tc.load();
    let target = gid(&ws, "src/thing.rs", "Thing::compute_total");
    let callers = ws.callers_of(target);
    assert_eq!(callers.len(), 1, "t.compute_total() should link by name");
    assert!(callers[0].1.by_name, "and be marked as a name-based match");
}

#[test]
fn common_std_method_names_are_never_linked_by_name() {
    let tc = TempCrate::new(&[
        ("Cargo.toml", "[package]\nname = \"m\"\nversion = \"0.1.0\"\n"),
        ("src/lib.rs", "pub mod thing;\npub fn go(v: &Vec<u32>) -> usize { v.len() }\n"),
        ("src/thing.rs", "pub struct Thing;\nimpl Thing { pub fn len(&self) -> usize { 0 } }\n"),
    ]);
    let ws = tc.load();
    let their_len = gid(&ws, "src/thing.rs", "Thing::len");
    assert!(ws.callers_of(their_len).is_empty(), "v.len() on a Vec must not bind to Thing::len");
}

#[test]
fn missing_module_file_is_reported() {
    let tc = TempCrate::new(&[("Cargo.toml", "[package]\nname = \"m\"\nversion = \"0.1.0\"\n"), ("src/lib.rs", "pub mod nowhere;\n")]);
    let ws = tc.load();
    assert!(ws.missing_modules.iter().any(|(_, n)| n == "nowhere"), "a `mod` with no file should be surfaced");
}

#[test]
fn binary_root_is_indexed_alongside_the_library() {
    let tc = small_crate();
    let ws = tc.load();
    assert!(ws.files.iter().any(|f| f.path.ends_with("main.rs")), "src/main.rs is a second crate root and must not be skipped");
}

#[test]
fn dependency_calls_stay_out_of_user_code() {
    let tc = small_crate();
    let ws = tc.load();
    for f in &ws.files {
        for r in &f.references {
            if r.provenance() == Provenance::ThirdParty {
                assert!(!r.to.is_user_code(), "{} was classified as library but counted as user code", r.path);
            }
        }
    }
}

// ------------------------------------------------------------------ output

#[test]
fn cfg_gives_each_function_its_own_cluster() {
    let src = "struct A; struct B;\nimpl A { fn new() -> A { A } }\nimpl B { fn new() -> B { B } }\n";
    let out = flowlens::cfg::emit_cfg_dot(src, &[]);
    assert_eq!(out.matches("subgraph cluster_").count(), 2);
    assert!(out.contains("cluster_new_0") && out.contains("cluster_new_1"));
}

fn assert_balanced(out: &str) {
    let mut depth = 0i32;
    let mut in_quotes = false;
    let mut escaped = false;
    for ch in out.chars() {
        match ch {
            _ if escaped => escaped = false,
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '{' if !in_quotes => depth += 1,
            '}' if !in_quotes => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0, "unbalanced braces");
    }
    assert_eq!(depth, 0, "unbalanced braces");
    assert!(!in_quotes, "unterminated string");
}

#[test]
fn call_graph_output_is_balanced() {
    let tc = small_crate();
    let ws = tc.load();
    for include_library in [false, true] {
        assert_balanced(&flowlens::dot::emit_call_graph(&ws, flowlens::dot::Options { include_library, ..Default::default() }));
    }
}

#[test]
fn file_graph_output_is_balanced() {
    let tc = small_crate();
    assert_balanced(&flowlens::dot::emit_file_graph(&tc.load()));
}

#[test]
fn call_graph_omits_library_nodes_by_default() {
    let tc = small_crate();
    let ws = tc.load();
    let plain = flowlens::dot::emit_call_graph(&ws, Default::default());
    assert!(!plain.contains("cluster_library"), "library calls are excluded unless asked for");
    let with_lib = flowlens::dot::emit_call_graph(&ws, flowlens::dot::Options { include_library: true, ..Default::default() });
    assert!(with_lib.contains("cluster_library"));
}

// ------------------------------------------------------------ doc comments

const DOCUMENTED: &str = r#"
/// Normalise a string for comparison.
///
/// Trims surrounding whitespace and lowercases the result, so that
/// `"  Foo "` and `"foo"` compare equal.
pub fn normalize(s: &str) -> String { s.trim().to_lowercase() }

// An ordinary comment is not documentation.
pub fn undocumented() -> u8 { 1 }

/// One line only
pub fn terse() -> u8 { 2 }

pub struct Widget;

impl Widget {
    /// Build a widget. Extra detail on the same line.
    pub fn new() -> Widget { Widget }
}
"#;

fn documented_index() -> FileIndex {
    index_file("doc.rs", DOCUMENTED)
}

#[test]
fn doc_comments_are_captured() {
    let ix = documented_index();
    let s = &ix.symbols[find(&ix, "normalize")];
    let doc = s.doc.as_deref().expect("normalize is documented");
    assert!(doc.starts_with("Normalise a string for comparison."));
    assert!(doc.contains("lowercases the result"), "later lines are kept too");
}

#[test]
fn ordinary_comments_are_not_documentation() {
    let ix = documented_index();
    let s = &ix.symbols[find(&ix, "undocumented")];
    assert_eq!(s.doc, None);
    assert!(!s.is_documented());
}

#[test]
fn doc_summary_is_the_first_sentence() {
    let ix = documented_index();
    assert_eq!(ix.symbols[find(&ix, "normalize")].doc_summary().as_deref(), Some("Normalise a string for comparison."));
    // A summary with no trailing period is still a summary.
    assert_eq!(ix.symbols[find(&ix, "terse")].doc_summary().as_deref(), Some("One line only"));
    // Two sentences on one line: only the first is the summary.
    assert_eq!(ix.symbols[find(&ix, "Widget::new")].doc_summary().as_deref(), Some("Build a widget."));
}

#[test]
fn doc_summary_is_none_without_a_doc() {
    let ix = documented_index();
    assert_eq!(ix.symbols[find(&ix, "undocumented")].doc_summary(), None);
}

#[test]
fn graph_nodes_carry_doc_summaries_and_tooltips() {
    let tc = TempCrate::new(&[
        ("Cargo.toml", "[package]\nname = \"d\"\nversion = \"0.1.0\"\n"),
        ("src/lib.rs", "/// Add two numbers together.\n///\n/// Detail that belongs in the tooltip.\npub fn add(a: u32, b: u32) -> u32 { a + b }\npub fn go() -> u32 { add(1, 2) }\n"),
    ]);
    let ws = tc.load();

    let with_docs = flowlens::dot::emit_call_graph(&ws, Default::default());
    assert!(with_docs.contains("Add two numbers together."), "the summary belongs on the node label");
    assert!(with_docs.contains("Detail that belongs in the tooltip."), "the full doc belongs in the tooltip");
    assert_balanced(&with_docs);

    let without = flowlens::dot::emit_call_graph(&ws, flowlens::dot::Options { include_library: false, show_docs: false });
    // The separator is only ever emitted into a label, so its absence is a
    // precise check — the tooltip legitimately still carries the same words.
    assert!(with_docs.contains("———"), "documented nodes get a rule above the summary");
    assert!(!without.contains("———"), "--no-docs keeps summaries off the labels");
    assert!(without.contains("Detail that belongs in the tooltip."), "but the tooltip keeps the full doc either way");
}

#[test]
fn doc_text_with_quotes_does_not_break_the_graph() {
    let tc = TempCrate::new(&[("Cargo.toml", "[package]\nname = \"q\"\nversion = \"0.1.0\"\n"), ("src/lib.rs", "/// Handles \"quoted\" text, a backslash \\\\, and braces { like this }.\npub fn tricky() {}\n")]);
    let ws = tc.load();
    assert_balanced(&flowlens::dot::emit_call_graph(&ws, Default::default()));
}

// ------------------------------------------------------- interactive page

/// Pull the embedded `const DATA = {...};` back out of the page.
fn embedded_json(page: &str) -> &str {
    let start = page.find("const DATA = ").expect("data block") + "const DATA = ".len();
    let rest = &page[start..];
    let end = rest.find(";\n").expect("end of data block");
    &rest[..end]
}

/// Enough of a JSON check to catch an unescaped quote or a stray control
/// character, without adding a parser dependency.
fn assert_json_well_formed(json: &str) {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in json.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_str => escaped = true,
            '"' => in_str = !in_str,
            '{' | '[' if !in_str => depth += 1,
            '}' | ']' if !in_str => {
                depth -= 1;
                assert!(depth >= 0, "unbalanced at byte {i}");
            }
            c if in_str && (c as u32) < 0x20 => {
                panic!("raw control character U+{:04X} inside a string at byte {i}", c as u32)
            }
            _ => {}
        }
    }
    assert_eq!(depth, 0, "unbalanced braces");
    assert!(!in_str, "unterminated string");
}

fn documented_crate() -> TempCrate {
    TempCrate::new(&[
        ("Cargo.toml", "[package]\nname = \"doc-demo\"\nversion = \"0.1.0\"\n"),
        (
            "src/lib.rs",
            "pub mod math;\n\
             use std::collections::HashMap;\n\
             use crate::math::add;\n\n\
             /// Run the whole thing.\n\
             ///\n\
             /// Longer detail that only the tooltip and detail pane show.\n\
             pub fn run(times: u32, label: &str) -> HashMap<String, u32> {\n\
             let mut m = HashMap::new();\n\
             m.insert(label.to_string(), add(times, 1));\n\
             m\n}\n",
        ),
        (
            "src/math.rs",
            "/// Add two numbers.\npub fn add(a: u32, b: u32) -> u32 { a + b }\n\
             pub fn unused() {}\n",
        ),
    ])
}

#[test]
fn parameters_are_captured_with_names_and_types() {
    let tc = documented_crate();
    let ws = tc.load();
    let run = ws.symbol(gid(&ws, "src/lib.rs", "run"));
    let got: Vec<(&str, &str)> = run.params.iter().map(|p| (p.name.as_str(), p.ty.as_str())).collect();
    assert_eq!(got, vec![("times", "u32"), ("label", "&str")]);
    assert_eq!(run.ret.as_deref(), Some("HashMap<String, u32>"));
}

#[test]
fn receiver_is_recorded_as_a_parameter() {
    let ix = index_file("r.rs", "pub struct S;\nimpl S { pub fn go(&mut self, n: u32) {} }\n");
    let go = &ix.symbols[find(&ix, "S::go")];
    assert_eq!(go.params[0].name, "self");
    assert_eq!(go.params[0].ty, "&mut self");
    assert_eq!(go.params[1].name, "n");
}

#[test]
fn html_page_is_self_contained() {
    let tc = documented_crate();
    let page = flowlens::html::emit(&tc.load());
    assert!(page.starts_with("<!doctype html>"));
    assert!(page.contains("<style>") && page.contains("<script>"));
    // No network: a strict reader should find no external references.
    assert!(!page.contains("http://") && !page.contains("https://"));
    assert!(!page.contains("<link ") && !page.contains("src=\""));
}

#[test]
fn html_data_block_is_valid_json() {
    let tc = documented_crate();
    let page = flowlens::html::emit(&tc.load());
    assert_json_well_formed(embedded_json(&page));
}

#[test]
fn html_carries_params_docs_and_calls() {
    let tc = documented_crate();
    let page = flowlens::html::emit(&tc.load());
    let json = embedded_json(&page);
    assert!(json.contains("\"name\":\"times\""), "parameter names");
    assert!(json.contains("\"ret\":\"HashMap"), "return type");
    assert!(json.contains("Run the whole thing."), "doc summary");
    assert!(json.contains("Longer detail"), "the full doc, not just the summary");
    assert!(json.contains("\"calls\":["), "call edges");
    assert!(json.contains("\"lib\":["), "library calls, kept separate");
}

#[test]
fn angle_brackets_in_docs_cannot_break_out_of_the_script() {
    // A doc comment containing `</script>` would end the tag early and inject
    // markup into the page.
    let tc = TempCrate::new(&[("Cargo.toml", "[package]\nname = \"x\"\nversion = \"0.1.0\"\n"), ("src/lib.rs", "/// Dangerous: </script><img onerror=alert(1)> and a \"quote\".\npub fn f() {}\n")]);
    let page = flowlens::html::emit(&tc.load());
    let json = embedded_json(&page);
    assert!(!json.contains("</script>"), "the closing tag must be escaped");
    assert!(json.contains("\\u003c"), "angle brackets escape to \\u003c");
    assert_json_well_formed(json);
    // Exactly one script element.
    assert_eq!(page.matches("</script>").count(), 1);
}

#[test]
fn html_output_is_deterministic() {
    let tc = documented_crate();
    let ws = tc.load();
    assert_eq!(flowlens::html::emit(&ws), flowlens::html::emit(&ws));
}

#[test]
fn html_includes_every_callable_but_no_types() {
    let tc = documented_crate();
    let ws = tc.load();
    let page = flowlens::html::emit(&ws);
    let json = embedded_json(&page);
    let callables: usize = ws.files.iter().map(|f| f.callables().count()).sum();
    assert_eq!(json.matches("\"qualified\":").count(), callables);
    // `unused` has no callers but must still be listed.
    assert!(json.contains("\"name\":\"unused\""));
}

// ------------------------------------------------------ call-site detail

const FLOW: &str = r#"
fn other() -> u32 { 1 }
fn pair() -> (u32, u32) { (1, 2) }
fn load(n: u32) -> Option<u32> { Some(n) }
fn helper(a: u32, b: &str) -> String { String::new() }

fn run(total: &mut u32) {
    let name = helper(other(), "hi");
    let n: u32 = other();
    let (a, b) = pair();
    *total += other();
    helper(1, "discarded");
    if other() == 2 {}
    let w = load(3).unwrap();
}
"#;

fn flow_index() -> FileIndex {
    index_file("flow.rs", FLOW)
}

fn call_at<'a>(ix: &'a FileIndex, line: usize, name: &str) -> &'a Reference {
    ix.references.iter().find(|r| r.loc.line == line && r.name == name && r.is_call()).unwrap_or_else(|| panic!("no call to {name} on line {line}"))
}

#[test]
fn call_site_arguments_are_captured_as_written() {
    let ix = flow_index();
    assert_eq!(call_at(&ix, 8, "helper").args, vec!["other()".to_string(), "\"hi\"".to_string()]);
    assert!(call_at(&ix, 9, "other").args.is_empty());
}

#[test]
fn let_binding_is_recorded() {
    let ix = flow_index();
    assert_eq!(call_at(&ix, 8, "helper").binding.as_deref(), Some("name"));
    assert_eq!(call_at(&ix, 9, "other").binding.as_deref(), Some("n"));
}

#[test]
fn destructuring_binding_is_kept_whole() {
    let ix = flow_index();
    assert_eq!(call_at(&ix, 10, "pair").binding.as_deref(), Some("(a, b)"));
}

#[test]
fn assignment_records_its_target() {
    let ix = flow_index();
    assert_eq!(call_at(&ix, 11, "other").binding.as_deref(), Some("*total"));
}

#[test]
fn a_call_used_as_an_argument_binds_nothing() {
    let ix = flow_index();
    // `other()` inside `helper(other(), "hi")` is not stored anywhere.
    let inner = ix.references.iter().find(|r| r.loc.line == 8 && r.name == "other").expect("the nested other() call");
    assert_eq!(inner.binding, None);
}

#[test]
fn discarded_and_conditional_calls_bind_nothing() {
    let ix = flow_index();
    assert_eq!(call_at(&ix, 12, "helper").binding, None, "statement position");
    assert_eq!(call_at(&ix, 13, "other").binding, None, "an if condition");
}

#[test]
fn value_flowing_through_postfix_calls_keeps_its_binding() {
    let ix = flow_index();
    // `let w = load(3).unwrap();` — the value still ends up in `w`.
    assert_eq!(call_at(&ix, 14, "load").binding.as_deref(), Some("w"));
    assert_eq!(call_at(&ix, 14, "load").args, vec!["3".to_string()]);
}

#[test]
fn html_calls_carry_arguments_and_bindings() {
    let tc = documented_crate();
    let page = flowlens::html::emit(&tc.load());
    let json = embedded_json(&page);
    assert!(json.contains("\"args\":["), "call sites record their arguments");
    assert!(json.contains("\"bind\":"), "and where the result is stored");
    assert_json_well_formed(json);
}

#[test]
fn call_order_is_preserved_for_the_flow_view() {
    // The page sorts by line, so the data must carry an honest line for each
    // call — not one line per target.
    let tc = TempCrate::new(&[
        ("Cargo.toml", "[package]\nname = \"o\"\nversion = \"0.1.0\"\n"),
        (
            "src/lib.rs",
            "pub fn a() {}\npub fn b() {}\npub fn c() {}\n\
             pub fn run() {\n    c();\n    a();\n    b();\n    a();\n}\n",
        ),
    ]);
    let ws = tc.load();
    let f = &ws.files[0];
    let run = f.symbols.iter().find(|s| s.name == "run").unwrap();
    let mut calls: Vec<(usize, &str)> = f.callees_of(run.id).into_iter().filter(|r| r.is_call() && r.to.is_user_code()).map(|r| (r.loc.line, r.name.as_str())).collect();
    calls.sort();
    assert_eq!(calls, vec![(5, "c"), (6, "a"), (7, "b"), (8, "a")], "each call site keeps its own line, including the repeat of `a`");
}

// ------------------------------------------------ nothing silently dropped

/// One call whose arguments are far longer than any sensible display width.
const LONG_ARGS: &str = r#"
pub mod svc { pub fn run() {} }
fn register(handlers: Vec<fn()>, name: String, retries: u32, description: &str) {}
fn setup() {
    register(
        vec![svc::run],
        "calculation_generation_service::run".to_string(),
        3,
        "recalculates every derived figure for the current accounting period",
    );
}
"#;

#[test]
fn long_arguments_are_stored_in_full() {
    let ix = index_file("long.rs", LONG_ARGS);
    let call = ix.references.iter().find(|r| r.name == "register" && r.is_call()).expect("the register() call");
    assert_eq!(call.args.len(), 4);
    assert_eq!(call.args[0], "vec![svc::run]");
    assert_eq!(call.args[1], "\"calculation_generation_service::run\".to_string()", "no ellipsis, no cut");
    assert_eq!(call.args[3], "\"recalculates every derived figure for the current accounting period\"");
    for a in &call.args {
        assert!(!a.contains('\u{2026}'), "argument was truncated: {a}");
    }
}

#[test]
fn long_bindings_are_stored_in_full() {
    let ix = index_file(
        "b.rs",
        "fn make() -> (u32, u32, u32) { (1, 2, 3) }\n\
         fn f() { let (first_component, second_component, third_component) = make(); }\n",
    );
    let call = ix.references.iter().find(|r| r.name == "make").unwrap();
    assert_eq!(call.binding.as_deref(), Some("(first_component, second_component, third_component)"));
}

#[test]
fn the_page_carries_long_arguments_verbatim() {
    let tc = TempCrate::new(&[("Cargo.toml", "[package]\nname = \"L\"\nversion = \"0.1.0\"\n"), ("src/lib.rs", LONG_ARGS)]);
    let page = flowlens::html::emit(&tc.load());
    let json = embedded_json(&page);
    // The argument the user saw cut off must survive into the page intact.
    assert!(json.contains("recalculates every derived figure for the current accounting period"), "the last argument must reach the page whole");
    assert!(!json.contains('\u{2026}'), "no ellipsis in the data");
    assert_json_well_formed(json);
}

#[test]
fn long_steps_are_marked_expandable() {
    let page = flowlens::html::emit(&TempCrate::new(&[("Cargo.toml", "[package]\nname = \"L\"\nversion = \"0.1.0\"\n"), ("src/lib.rs", LONG_ARGS)]).load());
    // The clipping is CSS, and the expanded state has to be reachable.
    assert!(page.contains("text-overflow: ellipsis"), "collapsed view clips");
    assert!(page.contains("li.open > .step-body"), "expanded view exists");
    assert!(page.contains("expandAll"), "expand-all control exists");
    assert!(page.contains("more"), "an affordance says there is more to see");
}

// -------------------------------------------------------- control flow

const NESTED: &str = r#"
fn a() {} fn b() {} fn c() {} fn d() {} fn e() {} fn g() {}
fn run(items: Vec<u32>, flag: bool) {
    a();
    for it in &items {
        b();
        if flag { c(); } else { d(); }
    }
    while flag { e(); }
    match flag { true => g(), false => a() }
    let f = || b();
}
"#;

fn nested_index() -> FileIndex {
    index_file("n.rs", NESTED)
}

fn ctx_of<'a>(ix: &'a FileIndex, line: usize, name: &str) -> &'a [ControlFrame] {
    &call_at(ix, line, name).context
}

#[test]
fn a_top_level_call_has_no_control_context() {
    let ix = nested_index();
    assert!(ctx_of(&ix, 4, "a").is_empty());
    assert!(!call_at(&ix, 4, "a").in_loop());
    assert!(!call_at(&ix, 4, "a").is_conditional());
}

#[test]
fn a_call_in_a_for_loop_is_marked_as_repeating() {
    let ix = nested_index();
    let ctx = ctx_of(&ix, 6, "b");
    assert_eq!(ctx.len(), 1);
    assert_eq!(ctx[0].kind, ControlKind::For);
    assert_eq!(ctx[0].label, "for it in &items");
    assert!(call_at(&ix, 6, "b").in_loop());
}

#[test]
fn nested_context_is_recorded_outermost_first() {
    let ix = nested_index();
    let ctx = ctx_of(&ix, 7, "c");
    let kinds: Vec<ControlKind> = ctx.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, vec![ControlKind::For, ControlKind::If]);
    let call = call_at(&ix, 7, "c");
    assert!(call.in_loop() && call.is_conditional());
}

#[test]
fn the_else_branch_is_distinguished_from_the_if() {
    let ix = nested_index();
    let c = ctx_of(&ix, 7, "c");
    let d = ctx_of(&ix, 7, "d");
    assert_eq!(c.last().unwrap().kind, ControlKind::If);
    assert_eq!(d.last().unwrap().kind, ControlKind::Else, "the else branch must not be reported as the if branch");
}

#[test]
fn while_loops_both_repeat_and_are_conditional() {
    let ix = nested_index();
    let call = call_at(&ix, 9, "e");
    assert_eq!(call.context[0].kind, ControlKind::While);
    assert!(call.in_loop());
    assert!(call.is_conditional());
}

#[test]
fn match_arms_carry_their_pattern() {
    let ix = nested_index();
    let g = ctx_of(&ix, 10, "g");
    assert_eq!(g[0].kind, ControlKind::MatchArm);
    assert_eq!(g[0].label, "true");
    assert!(call_at(&ix, 10, "g").is_conditional());
}

#[test]
fn a_call_in_a_closure_is_neither_loop_nor_branch() {
    let ix = nested_index();
    let call = call_at(&ix, 11, "b");
    assert_eq!(call.context[0].kind, ControlKind::Closure);
    assert!(!call.in_loop() && !call.is_conditional());
}

#[test]
fn context_stops_at_the_function_boundary() {
    // The outer `for` belongs to `outer`, not to the call inside `inner`.
    let ix = index_file("s.rs", "fn t() {}\nfn outer() { for _ in 0..3 { fn inner() { t(); } } }\n");
    let call = call_at(&ix, 2, "t");
    assert!(call.context.is_empty(), "a nested fn starts a fresh context, got {:?}", call.context);
}

#[test]
fn the_page_carries_the_control_context() {
    let tc = TempCrate::new(&[("Cargo.toml", "[package]\nname = \"N\"\nversion = \"0.1.0\"\n"), ("src/lib.rs", NESTED)]);
    let page = flowlens::html::emit(&tc.load());
    let json = embedded_json(&page);
    assert!(json.contains("\"ctx\":["), "calls carry their context");
    assert!(json.contains("\"k\":\"for\""), "loops are labelled");
    // `&` is escaped to \u0026 so it can never close the script tag early.
    assert!(json.contains("for it in "), "with the header as written");
    assert!(json.contains("\\u0026items"), "and its ampersand safely escaped");
    assert_json_well_formed(json);
    // And the page can draw them.
    assert!(page.contains("renderFlow"), "nesting renderer present");
    assert!(page.contains("runs repeatedly"), "loops are called out");
}

#[test]
fn the_page_offers_hiding_and_remembers_it() {
    let tc = documented_crate();
    let page = flowlens::html::emit(&tc.load());
    assert!(page.contains("data-hide-fn"), "per-function hide");
    assert!(page.contains("data-hide-file"), "per-file hide");
    assert!(page.contains("showAll"), "a way back");
    assert!(page.contains("localStorage"), "the choice persists");
    assert!(page.contains("isHidden"), "hidden items are filtered out");
}
