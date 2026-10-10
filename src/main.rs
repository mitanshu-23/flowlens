//! flowlens — see how your Rust code flows.
//!
//! Point it at a crate (or a Cargo workspace) and it writes one page: a map
//! of every feature, what it calls in order, and the steps each function
//! takes, in plain words. With git history, the page also shows what a change
//! did, feature by feature.

use std::path::Path;
use std::process::ExitCode;

use flowlens::review;

const USAGE: &str = "\
flowlens — see how your Rust code flows, and what a change did to it

USAGE
    flowlens <PATH> --review [OUT.html] [--against <REF>]

    PATH   a crate directory (one holding Cargo.toml or src/), a Cargo
           workspace root, or a .rs file. The `mod` tree is followed from the
           crate root and calls are resolved across files.

OPTIONS
    --review [OUT.html] write the flow page: every feature, what it calls, and
                        the steps each function takes. Writes review.html
                        unless a name is given.
    --against <REF>     what the page compares with: a commit, branch or tag
                        (default HEAD, the last commit — so it shows your
                        uncommitted changes). With no git history the page
                        shows the project as it is.
    -h, --help          this text

EXAMPLES
    flowlens . --review                   the flow, and what changed since the last commit
    flowlens . --review --against main    everything changed on this branch
    flowlens . --review out/flow.html     write it somewhere else
";

#[derive(Default)]
struct Args {
    path: Option<String>,
    review: Option<String>,
    against: Option<String>,
    /// The path came straight after `--review`, where a page name could go.
    path_after_review: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1).peekable();
    // A word straight after `--review` that is not an .html name: if it turns
    // out to be a second path, it was most likely meant as the page's name.
    let mut after_review: Option<String> = None;
    while let Some(arg) = it.next() {
        let just_after_review = after_review.take();
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--review" => {
                // The output name is optional: take the next word only when it
                // is clearly a file name, not the input path.
                let out = it.next_if(|n| n.ends_with(".html") || n.ends_with(".htm"));
                if out.is_none() {
                    after_review = it.peek().filter(|n| !n.starts_with('-')).cloned();
                }
                a.review = Some(out.unwrap_or_else(|| "review.html".to_string()));
            }
            "--against" => a.against = Some(it.next().ok_or("--against needs a commit, branch or tag")?),
            other if other.starts_with('-') => return Err(format!("unknown option: {other}")),
            other => {
                if let Some(first) = &a.path {
                    // `. --review out.txt` or `--review out.txt .`: one of the
                    // two was meant as the page's name.
                    let name = if just_after_review.is_some() { Some(other) } else if a.path_after_review { Some(first.as_str()) } else { None };
                    return Err(match name {
                        Some(name) => format!("the review page's name must end in .html: `{name}`"),
                        None => format!("give one path to analyze, not two: `{first}` and `{other}`"),
                    });
                }
                a.path_after_review = just_after_review.is_some();
                a.path = Some(other.to_string());
            }
        }
    }
    a.path.as_ref().ok_or("no input path given")?;
    if a.review.is_none() {
        return Err("nothing to do: add --review to write the flow page".into());
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

    let out = args.review.as_deref().unwrap_or("review.html");
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
    ExitCode::SUCCESS
}
