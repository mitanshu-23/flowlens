//! Tests for the before/after review: the steps read out of a function, how
//! two versions line up, and which features count as changed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use flowlens::review::compare::Mark;
use flowlens::review::flow::{self, BlockKind, ExitKind, Step};
use flowlens::review::{self, Review, Status};
use flowlens::workspace::Workspace;

const MANIFEST: &str = "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n";

/// A throwaway directory, removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("flowlens-review-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn crate_with(main: &str) -> TempDir {
    let dir = TempDir::new();
    dir.write("Cargo.toml", MANIFEST);
    dir.write("src/main.rs", main);
    dir
}

/// The steps of `name` in a one-file crate.
fn steps_of(src: &str, name: &str) -> Vec<Step> {
    let dir = crate_with(src);
    let ws = Workspace::load(&dir.0).unwrap();
    let keys = flow::function_keys(&ws, "crate");
    let mut flows = flow::flows(&ws, &keys);
    flows.remove(&format!("crate::{name}")).expect("function exists").steps
}

fn compare(before: &str, after: &str) -> Review {
    let (b, a) = (crate_with(before), crate_with(after));
    let wb = Workspace::load(&b.0).unwrap();
    let wa = Workspace::load(&a.0).unwrap();
    review::compare(Some(&wb), &wa, "test".into())
}

fn feature<'a>(r: &'a Review, key: &str) -> &'a review::Feature {
    r.features.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("no feature {key}; have {:?}", r.features.iter().map(|f| &f.key).collect::<Vec<_>>()))
}

// ------------------------------------------------------------------ steps

#[test]
fn a_function_passed_by_name_is_handed_over_not_called() {
    let src = r#"
fn get<F>(f: F) -> F { f }
fn handler() {}
fn block() {}
fn routes() {
    let block = 1;
    let _ = block;
    get(handler);
    handler();
}
fn main() { routes(); }
"#;
    let steps = steps_of(src, "routes");
    let calls: Vec<(&str, Option<&str>)> = steps
        .iter()
        .filter_map(|s| match s {
            Step::Call { target, handed, .. } => Some((target.as_str(), handed.as_deref())),
            _ => None,
        })
        .collect();
    // `get` itself is called; `handler` is passed to it, then called; the
    // local `block` is a variable, not the function of that name.
    assert_eq!(calls, vec![("crate::handler", Some("get")), ("crate::get", None), ("crate::handler", None)]);
}

#[test]
fn a_change_to_a_handed_over_function_reaches_its_feature() {
    let before = "fn on(_: fn()) {}\nfn handler() {}\nfn main() { on(handler); }\n";
    let after = "fn on(_: fn()) {}\nfn handler() { let _ = 1; }\nfn main() { on(handler); }\n";
    let r = compare(before, after);
    assert_eq!(feature(&r, "crate::main").changed_inside, vec!["crate::handler".to_string()]);
    assert!(r.features.iter().all(|f| f.key != "crate::handler"), "a handler is reached, not a feature of its own");
}

#[test]
fn early_exits_are_steps_even_with_no_calls() {
    let steps = steps_of(
        r#"
fn check(p: &str) -> Result<(), String> {
    if p.is_empty() {
        return Err("empty".into());
    }
    let n = p.parse::<u32>().map_err(|e| e.to_string())?;
    let Some(c) = p.chars().next() else { return Ok(()) };
    if n > 10 { panic!("too big") }
    let _ = c;
    Ok(())
}
fn main() { check("1").unwrap(); }
"#,
        "check",
    );
    let shape: Vec<String> = steps
        .iter()
        .map(|s| match s {
            Step::Block { kind, steps, .. } => format!("{kind:?}[{}]", steps.len()),
            Step::Exit { kind, .. } => format!("{kind:?}"),
            Step::Call { target, .. } => target.clone(),
            Step::Set { var, .. } => format!("set {var}"),
        })
        .collect();
    assert_eq!(shape, ["If[1]", "Fail", "LetElse[1]", "If[1]"]);
    assert!(matches!(&steps[0], Step::Block { steps, .. }
        if matches!(&steps[0], Step::Exit { kind: ExitKind::Fail, .. })));
    assert!(matches!(&steps[3], Step::Block { steps, .. }
        if matches!(&steps[0], Step::Exit { kind: ExitKind::Crash, .. })));
}

#[test]
fn question_mark_and_unwrap_attach_to_the_users_own_call() {
    let steps = steps_of(
        r#"
fn load() -> Result<u8, ()> { Ok(1) }
fn run() -> Result<(), ()> {
    let a = load()?;
    let b = load().unwrap();
    let _ = (a, b);
    Ok(())
}
fn main() { run().unwrap(); }
"#,
        "run",
    );
    assert_eq!(steps.len(), 2, "{steps:?}");
    assert!(matches!(&steps[0], Step::Call { fallible: true, crashes: false, .. }));
    assert!(matches!(&steps[1], Step::Call { fallible: false, crashes: true, .. }));
}

#[test]
fn trailing_err_is_an_exit_but_trailing_ok_is_not() {
    let steps = steps_of(
        r#"
fn pick(n: u8) -> Result<u8, String> {
    if n == 0 { Err("zero".into()) } else { Ok(n) }
}
fn main() { let _ = pick(1); }
"#,
        "pick",
    );
    assert!(
        matches!(&steps[..], [Step::Block { kind: BlockKind::If, steps, .. }]
            if matches!(&steps[..], [Step::Exit { kind: ExitKind::Fail, .. }])),
        "{steps:?}"
    );
}

#[test]
fn else_if_chains_are_siblings_and_keep_their_anchors() {
    let steps = steps_of(
        r#"
fn a() {} fn b() {}
fn route(n: u8) {
    if n == 0 { a() } else if n == 1 { println!("one") } else { b() }
}
fn main() { route(0); }
"#,
        "route",
    );
    let kinds: Vec<BlockKind> = steps
        .iter()
        .map(|s| match s {
            Step::Block { kind, .. } => *kind,
            other => panic!("expected a block, got {other:?}"),
        })
        .collect();
    // The `else if` does nothing of note, but stays: the `else` after it is
    // the alternative to both conditions, and says so.
    assert_eq!(kinds, [BlockKind::If, BlockKind::ElseIf, BlockKind::Else]);
}

#[test]
fn library_calls_are_not_steps() {
    let steps = steps_of("fn main() { let v: Vec<u8> = Vec::new(); println!(\"{}\", v.len()); }", "main");
    assert!(steps.is_empty(), "{steps:?}");
}

// ---------------------------------------------------------------- compare

const BEFORE: &str = r#"
fn read(p: &str) -> String { p.to_string() }
fn save(_s: String) {}
fn upload(p: &str) {
    let s = read(p);
    save(s);
}
fn report() { read("x"); }
fn main() { upload("a"); }
#[cfg(test)]
mod tests { #[test] fn t() { super::upload("a"); } }
"#;

#[test]
fn an_added_early_exit_shows_as_a_change_to_the_feature() {
    let after = BEFORE.replace("    let s = read(p);\n", "    if p.len() > 10 { return; }\n    let s = read(p);\n");
    let r = compare(BEFORE, &after);

    let upload = &r.fns["crate::upload"];
    assert_eq!(upload.status, Status::Changed);
    assert_eq!(upload.rows[0].mark, Mark::Added);
    assert!(upload.rows[1..].iter().all(|row| row.mark == Mark::Same));

    // `main` reaches `upload`, so the feature changed — through it.
    let main = feature(&r, "crate::main");
    assert_eq!(main.status, Status::Changed);
    assert_eq!(main.changed_inside, ["crate::upload"]);

    // `report` shares `read` with `upload` but did not change.
    assert_eq!(feature(&r, "crate::report").status, Status::Same);
}

#[test]
fn identical_code_changes_nothing() {
    let r = compare(BEFORE, &BEFORE.replace("fn save", "/// Saves.\nfn save"));
    assert!(r.features.iter().all(|f| f.status == Status::Same));
    assert!(r.unreached.is_empty() && r.tests.is_empty());
}

#[test]
fn a_change_with_no_new_steps_is_flagged_as_details_only() {
    let r = compare(BEFORE, &BEFORE.replace("p.to_string()", "p.to_uppercase()"));
    let read = &r.fns["crate::read"];
    assert_eq!(read.status, Status::Changed);
    assert!(read.details_only());
    assert_eq!(feature(&r, "crate::report").status, Status::Changed);
}

#[test]
fn a_changed_condition_pairs_the_branch_instead_of_listing_it_twice() {
    let before = "fn a() {}\nfn main() { let n = 3; if n > 1 { a() } }";
    let after = "fn a() {}\nfn main() { let n = 3; if n > 2 { a() } }";
    let r = compare(before, after);
    let rows = &r.fns["crate::main"].rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].mark, Mark::Changed);
    assert_eq!(rows[0].children[0].mark, Mark::Same);
}

#[test]
fn tests_are_reported_separately_and_are_not_features() {
    let r = compare(BEFORE, &BEFORE.replace("super::upload(\"a\")", "super::upload(\"b\")"));
    assert_eq!(r.tests, ["crate::tests::t"]);
    assert!(r.features.iter().all(|f| f.key != "crate::tests::t"));
    // A test calling `upload` does not stop `main` from being its only caller.
    assert!(r.features.iter().all(|f| f.status == Status::Same));
}

#[test]
fn trait_methods_are_not_features() {
    let src = "struct S;\nimpl Drop for S { fn drop(&mut self) {} }\nfn main() { let _s = S; }";
    let r = compare(src, src);
    assert!(r.features.iter().all(|f| !f.key.ends_with("drop")));
}

#[test]
fn removed_and_new_features() {
    let r = compare(BEFORE, &BEFORE.replace("fn report() { read(\"x\"); }", "fn export() { save(read(\"y\")); }"));
    assert_eq!(feature(&r, "crate::report").status, Status::Removed);
    assert_eq!(feature(&r, "crate::export").status, Status::Added);
}

// -------------------------------------------------------------------- git

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git").current_dir(dir).args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"]).args(args).output().expect("git runs").status.success();
    assert!(ok, "git {args:?} failed");
}

#[test]
fn review_compares_the_working_tree_with_the_last_commit() {
    let dir = crate_with(BEFORE);
    git(&dir.0, &["init", "-q"]);
    git(&dir.0, &["add", "-A"]);
    git(&dir.0, &["commit", "-qm", "first"]);
    dir.write("src/main.rs", &BEFORE.replace("save(s);", "save(s);\n    save(read(p));"));

    let r = review::run(&dir.0, "HEAD").expect("review runs");
    assert!(r.against.starts_with("HEAD ("), "{}", r.against);
    assert_eq!(feature(&r, "crate::main").status, Status::Changed);
    assert!(r.fns["crate::upload"].rows.iter().any(|row| row.mark == Mark::Added));

    // The page embeds it all and stays self-contained.
    let html = review::page::emit(&r);
    assert!(html.contains("const DATA = {"));
    assert!(!html.contains("<script src"));
}

#[test]
fn review_without_any_commit_shows_the_project_and_says_why() {
    let dir = crate_with(BEFORE);
    git(&dir.0, &["init", "-q"]);
    let r = review::run(&dir.0, "HEAD").expect("no commits is not an error");
    assert!(!r.has_history);
    assert!(r.against.contains("no commits"), "{}", r.against);
}

#[test]
fn the_map_knows_each_call_before_and_now() {
    let r = compare(BEFORE, &BEFORE.replace("    save(s);\n", "    let _ = s;\n"));
    let upload = &r.fns["crate::upload"];
    assert_eq!(upload.calls_before, ["crate::read", "crate::save"]);
    assert_eq!(upload.calls_now, ["crate::read"]);
    assert!(r.fns["crate::main"].is_feature);
    assert!(!upload.is_feature);
    // Nothing calls `save` any more: it has become an entry point of its own.
    assert!(r.fns["crate::save"].is_feature);
}

// ----------------------------------------------------- workspaces, no git

#[test]
fn a_folder_without_git_shows_the_project_as_it_is() {
    let dir = crate_with(BEFORE);
    let r = review::run(&dir.0, "HEAD").expect("no git is not an error");
    assert!(!r.has_history);
    assert!(r.against.contains("not in git"), "{}", r.against);
    assert!(r.fns.values().all(|f| f.status == Status::Same));
    assert!(feature(&r, "crate::main").status == Status::Same);
}

#[test]
fn a_bad_reference_is_still_an_error() {
    let dir = crate_with(BEFORE);
    git(&dir.0, &["init", "-q"]);
    git(&dir.0, &["add", "-A"]);
    git(&dir.0, &["commit", "-qm", "first"]);
    let err = review::run(&dir.0, "no-such-branch").err().expect("typo in --against");
    assert!(err.contains("cannot find"), "{err}");
}

#[test]
fn a_cargo_workspace_reviews_every_member_on_one_map() {
    let dir = TempDir::new();
    dir.write("Cargo.toml", "[workspace]\nresolver = \"2\"\nmembers = [\n  \"crates/api\",\n  \"crates/worker\",\n]\n");
    dir.write("crates/api/Cargo.toml", "[package]\nname = \"api\"\nversion = \"0.1.0\"\n");
    dir.write("crates/api/src/main.rs", "fn serve() {}\nfn main() { serve(); }\n");
    dir.write("crates/worker/Cargo.toml", "[package]\nname = \"worker\"\nversion = \"0.1.0\"\n");
    dir.write("crates/worker/src/main.rs", "fn work() {}\nfn main() { work(); }\n");
    git(&dir.0, &["init", "-q"]);
    git(&dir.0, &["add", "-A"]);
    git(&dir.0, &["commit", "-qm", "first"]);
    dir.write("crates/worker/src/main.rs", "fn work() {}\nfn retry() {}\nfn main() { work(); retry(); }\n");

    let r = review::run(&dir.0, "HEAD").expect("workspace loads");
    assert!(r.has_history);
    assert_eq!(feature(&r, "api::main").status, Status::Same);
    assert_eq!(feature(&r, "worker::main").status, Status::Changed);
    assert_eq!(r.fns["worker::retry"].status, Status::Added);
    assert_eq!(r.fns["worker::main"].file, "crates/worker/src/main.rs");
}

#[test]
fn workspace_globs_are_expanded() {
    let dir = TempDir::new();
    dir.write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
    for name in ["a", "b"] {
        dir.write(&format!("crates/{name}/Cargo.toml"), &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"));
        dir.write(&format!("crates/{name}/src/main.rs"), "fn main() {}\n");
    }
    let r = review::run(&dir.0, "HEAD").expect("loads without git");
    assert!(r.fns.contains_key("a::main") && r.fns.contains_key("b::main"));
}

#[test]
fn a_function_named_like_its_module_is_still_a_step() {
    // `mod reject;` plus `use reject::reject;` — the call must reach the
    // function, not stop at the module of the same name.
    let dir = TempDir::new();
    dir.write("Cargo.toml", MANIFEST);
    dir.write("src/main.rs", "pub mod reject;\nuse crate::reject::reject;\nfn handle() { reject(1); }\nfn main() { handle(); }\n");
    dir.write("src/reject.rs", "pub fn reject(_id: u32) {}\n");
    let ws = Workspace::load(&dir.0).unwrap();
    let r = review::compare(Some(&ws), &ws, "test".into());
    assert_eq!(r.fns["crate::handle"].calls_now, ["crate::reject::reject"]);
    assert!(r.features.iter().all(|f| f.key != "crate::reject::reject"));
}

#[test]
fn calls_with_type_arguments_are_steps() {
    let steps = steps_of("fn get<T: Default>(_k: &str) -> T { T::default() }\nfn main() { let _n: u8 = get::<u8>(\"x\"); }\n", "main");
    assert!(matches!(&steps[..], [Step::Call { target, .. }] if target == "crate::get"), "{steps:?}");
}

#[test]
fn a_pattern_knows_what_it_is_checked_against() {
    let steps = steps_of(
        r#"
fn load() -> Result<u8, String> { Ok(1) }
fn run(data: Option<u8>) -> Result<(), String> {
    match load() {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    let Some(_v) = data else { return Err("missing".into()) };
    Ok(())
}
fn main() { let _ = run(None); }
"#,
        "run",
    );
    // The arm checks the value of the user's own call just before it.
    assert!(
        matches!(&steps[1], Step::Block { kind: BlockKind::When, header, on_call: Some(c), .. }
        if header == "Err(e)" && c == "crate::load"),
        "{steps:?}"
    );
    // `let … else` keeps the pattern and the code it checks apart.
    assert!(
        matches!(&steps[2], Step::Block { kind: BlockKind::LetElse, header, on, on_call: None, .. }
        if header == "Some(_v)" && on == "data"),
        "{steps:?}"
    );
}

#[test]
fn a_flag_a_decision_reads_is_followed_through_the_function() {
    let steps = steps_of(
        r#"
fn work(items: &[u8]) {
    let mut ok = true;
    let total = items.len();
    for i in items {
        if *i == 0 {
            ok = false;
            break;
        }
    }
    if ok { done(total); }
}
fn done(_n: usize) {}
fn main() { work(&[1]); }
"#,
        "work",
    );
    // `ok` is read by a decision and changed: where it starts and where it is
    // set are steps. `total` is only passed along: it is not.
    assert!(matches!(&steps[0], Step::Set { var, op, value, .. } if var == "ok" && op == "let" && value == "true"), "{steps:?}");
    let Step::Block { steps: inner, .. } = &steps[1] else { panic!("{steps:?}") };
    let Step::Block { steps: branch, .. } = &inner[0] else { panic!("{inner:?}") };
    assert!(matches!(&branch[0], Step::Set { var, op, value, .. } if var == "ok" && op == "=" && value == "false"), "{branch:?}");
    assert!(!steps.iter().any(|s| matches!(s, Step::Set { var, .. } if var == "total")), "{steps:?}");
}

#[test]
fn break_and_continue_know_which_loop_they_act_on() {
    let steps = steps_of(
        r#"
fn work(rows: &[Vec<u8>]) {
    'rows: for row in rows {
        for cell in row {
            if *cell == 0 { continue 'rows; }
            if *cell == 9 { break; }
        }
    }
}
fn main() { work(&[]); }
"#,
        "work",
    );
    let Step::Block { steps: outer, .. } = &steps[0] else { panic!("{steps:?}") };
    let Step::Block { steps: inner, .. } = &outer[0] else { panic!("{outer:?}") };
    let exit = |i: usize| match &inner[i] {
        Step::Block { steps, .. } => match &steps[0] {
            Step::Exit { kind, detail, .. } => (*kind, detail.clone()),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    // `continue 'rows` acts on the loop one out; a bare `break` on its own.
    assert_eq!(exit(0), (ExitKind::Continue, "1".to_string()));
    assert_eq!(exit(1), (ExitKind::Break, "0".to_string()));
}

#[test]
fn an_if_with_nothing_to_show_but_an_else_reads_the_other_way_round() {
    let steps = steps_of(
        r#"
fn check(m: Option<u8>, n: u8) -> Result<(), String> {
    if let Some(_v) = m { let _ = 1; } else { return Err("missing".into()); }
    if n > 3 { let _ = 2; } else { return Err("small".into()); }
    Ok(())
}
fn main() { let _ = check(None, 1); }
"#,
        "check",
    );
    assert!(matches!(&steps[0], Step::Block { kind: BlockKind::LetElse, header, on, .. } if header == "Some(_v)" && on == "m"), "{steps:?}");
    assert!(matches!(&steps[1], Step::Block { kind: BlockKind::Unless, header, .. } if header == "n > 3"), "{steps:?}");
}

#[test]
fn nested_lookups_that_only_fail_read_the_other_way_round() {
    let steps = steps_of(
        r#"
fn set(doc: &mut serde_json::Value, store: &str, sku: Option<&str>) -> Result<(), String> {
    if let Some(s) = doc.get_mut("stock") {
        if let Some(by_store) = s.get_mut(store) {
            if let Some(sku) = sku {
                if let Some(item) = by_store.get_mut(sku) {
                    item["count"] = serde_json::Value::from(1);
                } else {
                    return Err("Unknown sku".to_string());
                }
            } else {
                return Err("Missing sku".to_string());
            }
        } else {
            return Err("Unknown store".to_string());
        }
    } else {
        return Err("Missing stock".to_string());
    }
    Ok(())
}
fn main() {}
"#,
        "set",
    );
    // The innermost lookup has nothing to show when it succeeds: it reads the
    // other way round. The three around it hold the nested checks, so their
    // `else`s stay — each the alternative to the lookup just above it.
    let dump = format!("{steps:?}");
    assert_eq!(dump.matches("kind: LetElse").count(), 1, "{steps:#?}");
    assert_eq!(dump.matches("kind: Else").count(), 3, "{steps:#?}");
}

/// The page is one file of hand-written script; a stray brace breaks all of
/// it without failing anything else. When Node is installed, check the
/// script parses.
#[test]
fn the_page_script_parses() {
    let Ok(out) = Command::new("node").arg("--version").output() else {
        eprintln!("node not installed; skipping the script check");
        return;
    };
    if !out.status.success() {
        return;
    }
    let r = compare(BEFORE, &BEFORE.replace("save(s);", "save(s);\n    save(read(p));"));
    let html = review::page::emit(&r);
    let start = html.find("<script>").expect("page has a script") + "<script>".len();
    let end = html.rfind("</script>").expect("script is closed");
    let dir = TempDir::new();
    let file = dir.0.join("page.js");
    fs::write(&file, &html[start..end]).unwrap();
    let check = Command::new("node").arg("--check").arg(&file).output().unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
}

#[test]
fn the_glossary_lists_types_fields_constants_and_who_uses_them() {
    let before = r#"
/// A unit of work.
#[derive(Debug)]
pub struct Job {
    /// What kind of job.
    pub kind: String,
}
const RETRIES: u32 = 5;
lazy_static::lazy_static! {
    /// Jobs waiting to run.
    static ref QUEUE: Vec<Job> = Vec::new();
}
fn run(job: Job) -> u32 { let _ = &QUEUE; let _ = job; RETRIES }
fn main() { run(Job { kind: String::new() }); }
"#;
    let after = before.replace("pub kind: String,", "pub kind: String,\n    pub year: i32,");
    let r = compare(before, &after);

    let job = &r.glossary["crate::Job"];
    assert_eq!(job.kind, "struct");
    assert_eq!(job.doc.as_deref(), Some("A unit of work."));
    let fields: Vec<(&str, &str)> = job.parts.iter().map(|p| (p.name.as_str(), p.ty.as_str())).collect();
    assert_eq!(fields, [("kind", "String"), ("year", "i32")]);
    assert_eq!(job.parts[0].doc.as_deref(), Some("What kind of job."));
    assert_eq!(job.status, Status::Changed, "a field was added");

    let retries = &r.glossary["crate::RETRIES"];
    assert_eq!((retries.ty.as_deref(), retries.value.as_deref()), (Some("u32"), Some("5")));
    assert_eq!(retries.used_by, ["crate::run"]);

    let queue = &r.glossary["crate::QUEUE"];
    assert_eq!(queue.kind, "static");
    assert_eq!(queue.doc.as_deref(), Some("Jobs waiting to run."));
    assert_eq!(queue.used_by, ["crate::run"]);

    let run = &r.fns["crate::run"];
    assert_eq!(run.params, [("job".to_string(), "Job".to_string())]);
    assert_eq!(run.ret.as_deref(), Some("u32"));
}

#[test]
fn a_function_knows_its_variables_where_they_start_and_change() {
    let dir = crate_with(
        r#"
fn work(items: &[u8]) -> bool {
    let mut ok: bool = true;
    let total = items.len();
    let mut tries = 3;
    for i in items {
        if *i == 0 { ok = false; }
        tries -= 1;
    }
    let later;
    later = total;
    let _ = (tries, later);
    ok
}
fn main() { work(&[1]); }
"#,
    );
    let ws = Workspace::load(&dir.0).unwrap();
    let r = review::compare(Some(&ws), &ws, "test".into());
    let vars = &r.fns["crate::work"].vars;
    let get = |n: &str| vars.iter().find(|v| v.name == n).unwrap_or_else(|| panic!("no {n} in {vars:?}"));
    let ok = get("ok");
    assert!(ok.mutable);
    assert_eq!((ok.ty.as_deref(), ok.init.as_deref()), (Some("bool"), Some("true")));
    assert_eq!(ok.sets.iter().map(|(_, op, v)| (op.as_str(), v.as_str())).collect::<Vec<_>>(), [("=", "false")]);
    assert_eq!(get("tries").sets[0].1, "-=");
    assert!(!get("total").mutable && get("total").sets.is_empty());
    assert_eq!(get("later").init, None);
    assert_eq!(get("later").sets.len(), 1);
}

#[test]
fn files_git_archive_would_leave_out_are_not_mistaken_for_new_ones() {
    // `export-ignore` keeps a file out of `git archive`; the "before" side
    // must still have it, or it shows as added though nothing changed.
    let dir = crate_with("mod helper;\nfn main() { helper::ok(); }\n");
    dir.write("src/helper.rs", "pub fn ok() {}\n");
    dir.write(".gitattributes", "src/helper.rs export-ignore\n");
    git(&dir.0, &["init", "-q"]);
    git(&dir.0, &["add", "-A"]);
    git(&dir.0, &["commit", "-qm", "first"]);

    let r = review::run(&dir.0, "HEAD").expect("review runs");
    assert!(r.fns.values().all(|f| f.status == Status::Same), "{:?}", r.fns.values().map(|f| (&f.key, f.status)).collect::<Vec<_>>());
}
