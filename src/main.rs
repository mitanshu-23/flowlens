//! flowlens — a static analyzer for Rust source.
//!
//! Point it at a file to index that file, or at a crate directory to follow the
//! `mod` tree and index the whole thing. Either way the output separates code
//! the user wrote from the standard library and dependencies: only the former
//! is a fact about their design.

use std::path::Path;
use std::process::ExitCode;

use flowlens::workspace::Workspace;
use flowlens::{cfg, dot, html, report, review};

const USAGE: &str = "\
flowlens — see what your code does now, next to what it did before

USAGE
    flowlens <PATH> --review [OUT.html] [--against <REF>]
    flowlens <PATH> [OPTIONS]

    PATH   a .rs file, or a crate directory (one holding Cargo.toml or src/).
           Given a directory, the `mod` tree is followed from the crate root
           and calls are resolved across files.

REVIEW
    --review [OUT.html] compare the code with an earlier version, feature by
                        feature: what each did before, what it does now.
                        Writes review.html unless a name is given.
    --against <REF>     what \"before\" means: a commit, branch or tag
                        (default HEAD, the last commit — so the review shows
                        your uncommitted changes)

EXPLORE
    --html <OUT.html>   write an interactive explorer — the readable option
    --dot <OUT.dot>     write the call graph instead of the report
    --files <OUT.dot>   write the file-dependency graph (best for large crates)
    --all               include std and dependency calls, which are
                        excluded by default
    --no-outline        skip the per-file definition listing
    --no-docs           leave doc summaries off the graph, for a compact layout
    --cfg <OUT.dot>     write per-function control-flow graphs
    --stats             print counts only
    -h, --help          this text

EXAMPLES
    flowlens . --review              what changed since the last commit
    flowlens . --review --against main
    flowlens . --html map.html       browse it all, params and docs included
    flowlens .                       whole crate, user-defined calls only
    flowlens src/index.rs            one file, in its crate's context
    flowlens . --files deps.dot      how the files depend on each other
    flowlens . --dot g.dot && dot -Tsvg g.dot -o g.svg
";

#[derive(Default)]
struct Args {
    path: Option<String>,
    dot: Option<String>,
    files: Option<String>,
    html: Option<String>,
    cfg: Option<String>,
    stats: bool,
    all: bool,
    no_outline: bool,
    no_docs: bool,
    review: Option<String>,
    against: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1).peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--stats" => a.stats = true,
            "--all" => a.all = true,
            "--no-outline" => a.no_outline = true,
            "--no-docs" => a.no_docs = true,
            "--dot" => a.dot = Some(it.next().ok_or("--dot needs a path")?),
            "--files" => a.files = Some(it.next().ok_or("--files needs a path")?),
            "--html" => a.html = Some(it.next().ok_or("--html needs a path")?),
            "--cfg" => a.cfg = Some(it.next().ok_or("--cfg needs a path")?),
            "--review" => {
                // The output name is optional: take the next word only when it
                // is clearly a file name, not the input path.
                let out = it.next_if(|n| n.ends_with(".html") || n.ends_with(".htm"));
                a.review = Some(out.unwrap_or_else(|| "review.html".to_string()));
            }
            "--against" => a.against = Some(it.next().ok_or("--against needs a commit, branch or tag")?),
            other if other.starts_with('-') => return Err(format!("unknown option: {other}")),
            other => a.path = Some(other.to_string()),
        }
    }
    a.path.as_ref().ok_or("no input path given")?;
    if a.against.is_some() && a.review.is_none() {
        return Err("--against only makes sense with --review".into());
    }
    Ok(a)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}\n");
            }
            eprint!("{USAGE}");
            return if msg.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
        }
    };

    let input = args.path.as_deref().unwrap_or(".");
    let path = Path::new(input);
    if !path.exists() {
        eprintln!("error: no such path: {input}");
        return ExitCode::FAILURE;
    }

    if let Some(out) = &args.review {
        let against = args.against.as_deref().unwrap_or("HEAD");
        let review = match review::run(path, against) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(e) = std::fs::write(out, review::page::emit(&review)) {
            eprintln!("error: cannot write {out}: {e}");
            return ExitCode::FAILURE;
        }
        if review.has_history {
            let changed = review.features.iter().filter(|f| f.status != review::Status::Same).count();
            eprintln!("review \u{2192} {out}   ({changed} of {} feature(s) changed, against {})", review.features.len(), review.against);
        } else {
            eprintln!("project map \u{2192} {out}   ({}, so there is nothing to compare with — showing the project as it is)", review.against);
        }
        eprintln!("open it: xdg-open {out}");
        return ExitCode::SUCCESS;
    }

    // A single file still gets crate context when there is a Cargo.toml above
    // it, so `use crate::db::connect` can be told apart from a dependency.
    let ws = match Workspace::open(path) {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if ws.files.is_empty() {
        eprintln!("error: found no Rust files under {input}");
        return ExitCode::FAILURE;
    }

    let mut wrote_something = false;

    if let Some(out) = &args.cfg {
        let src = &ws.files[0].src;
        let dot = cfg::emit_cfg_dot(src, &["println", "print", "eprintln", "format"]);
        if let Err(e) = std::fs::write(out, dot) {
            eprintln!("error: cannot write {out}: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("control-flow graph → {out}   (from {})", ws.files[0].path);
        wrote_something = true;
    }

    if let Some(out) = &args.dot {
        let opts = dot::Options { include_library: args.all, show_docs: !args.no_docs };
        if let Err(e) = std::fs::write(out, dot::emit_call_graph(&ws, opts)) {
            eprintln!("error: cannot write {out}: {e}");
            return ExitCode::FAILURE;
        }
        let n: usize = ws.files.iter().map(|f| f.callables().count()).sum();
        eprintln!("call graph → {out}   ({n} function(s), {} cross-file edge(s))", ws.linked);
        eprintln!("render: dot -Tsvg {out} -o {}.svg", out.trim_end_matches(".dot"));
        wrote_something = true;
    }

    if let Some(out) = &args.html {
        if let Err(e) = std::fs::write(out, html::emit(&ws)) {
            eprintln!("error: cannot write {out}: {e}");
            return ExitCode::FAILURE;
        }
        let n: usize = ws.files.iter().map(|f| f.callables().count()).sum();
        eprintln!("interactive explorer \u{2192} {out}   ({n} functions)");
        eprintln!("open it: xdg-open {out}");
        wrote_something = true;
    }

    if let Some(out) = &args.files {
        if let Err(e) = std::fs::write(out, dot::emit_file_graph(&ws)) {
            eprintln!("error: cannot write {out}: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("file graph → {out}   ({} file(s))", ws.files.len());
        eprintln!("render: dot -Tsvg {out} -o {}.svg", out.trim_end_matches(".dot"));
        wrote_something = true;
    }

    if args.stats {
        report::print_stats(&ws);
    } else if !wrote_something {
        report::print_report(&ws, report::Options { show_library: args.all, show_outline: !args.no_outline, show_docs: !args.no_docs });
    }

    ExitCode::SUCCESS
}
