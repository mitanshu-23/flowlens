//! Getting the "before" version of the code.
//!
//! The reader never sees git; it is only how the old code is fetched. The
//! committed tree at `REF` is read straight from git's object store —
//! `git ls-tree` for the file list, `git cat-file --batch` for the contents —
//! into a temporary directory. That leaves the user's repository, index and
//! working tree completely untouched (no checkout, no stash, no worktree
//! metadata), and unlike `git archive` it gives the files exactly as
//! committed: `export-ignore` and `export-subst` in `.gitattributes` do not
//! apply, so an ignored file is not mistaken for one the change added.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Why there is no "before" to fetch.
#[derive(Debug)]
pub enum Error {
    /// Not a git repository, or one with no commits: there is no history at
    /// all, which is a normal state for a project, not a mistake.
    NoHistory(String),
    /// Anything else: a bad reference, or git failing.
    Failed(String),
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Error::NoHistory(m) | Error::Failed(m) => m,
        }
    }
}

/// The old version of the code, on disk until dropped.
pub struct Snapshot {
    dir: PathBuf,
    /// Where the analyzed path sits inside the snapshot.
    pub path: PathBuf,
    /// What was compared against, for the page header.
    pub label: String,
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Export `reference` of the repository containing `path`.
pub fn snapshot(path: &Path, reference: &str) -> Result<Snapshot, Error> {
    let abs = path.canonicalize().map_err(|e| Error::Failed(format!("cannot read {}: {e}", path.display())))?;
    let cwd = if abs.is_dir() { abs.clone() } else { abs.parent().unwrap_or(Path::new("/")).to_path_buf() };

    let top = git(&cwd, &["rev-parse", "--show-toplevel"]).map_err(|_| Error::NoHistory("this folder is not in git yet".to_string()))?;
    let top = PathBuf::from(top.trim()).canonicalize().map_err(|e| Error::Failed(format!("cannot read the repository root: {e}")))?;

    let commit = git(&cwd, &["rev-parse", "--verify", "--quiet", &format!("{reference}^{{commit}}")]).map_err(|_| {
        let no_commits = git(&cwd, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_err();
        if no_commits { Error::NoHistory("this repository has no commits yet".to_string()) } else { Error::Failed(format!("cannot find `{reference}` in this repository")) }
    })?;
    let commit = commit.trim().to_string();
    let label = git(&cwd, &["log", "-1", "--format=%h · %s · %cs", &commit]).map(|s| s.trim().to_string()).unwrap_or_else(|_| commit.chars().take(7).collect());

    let dir = std::env::temp_dir().join(format!("flowlens-before-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::create_dir_all(&dir).map_err(|e| Error::Failed(format!("cannot create a temporary directory: {e}")))?;
    let snap = Snapshot { path: dir.join(abs.strip_prefix(&top).unwrap_or(Path::new(""))), dir, label: format!("{reference} ({label})") };

    export(&top, &commit, &snap.dir).map_err(|e| Error::Failed(format!("could not read `{reference}` from the repository: {e}")))?;
    Ok(snap)
}

/// Is this committed file one the analysis reads? Rust sources and manifests;
/// nothing else is needed to rebuild the "before" side.
fn wanted(path: &str) -> bool {
    path.ends_with(".rs") || path == "Cargo.toml" || path.ends_with("/Cargo.toml")
}

/// Write the files of `commit` that the analysis reads into `dir`, exactly as
/// committed.
fn export(top: &Path, commit: &str, dir: &Path) -> Result<(), String> {
    // Every file of the tree: `<mode> <type> <sha>\t<path>`, NUL-separated.
    let listing = Command::new("git").current_dir(top).args(["ls-tree", "-r", "-z", "--full-tree", commit]).stderr(Stdio::null()).output().map_err(|e| format!("cannot run git: {e}"))?;
    if !listing.status.success() {
        return Err("git ls-tree failed".into());
    }
    let mut files: Vec<(String, String)> = Vec::new();
    for entry in listing.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let Some((meta, path)) = entry.split_once('\t') else { continue };
        let mut parts = meta.split_whitespace();
        let (mode, kind, sha) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        // Plain files only: no symlinks (120000), no submodules (commit).
        if kind == "blob" && mode != "120000" && wanted(path) {
            files.push((sha.to_string(), path.to_string()));
        }
    }
    if files.is_empty() {
        return Ok(());
    }

    // All contents in one `cat-file --batch` round: each answer is
    // `<sha> blob <size>\n<bytes>\n`, in the order asked.
    let mut cat = Command::new("git").current_dir(top).args(["cat-file", "--batch"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("cannot run git: {e}"))?;
    let mut stdin = cat.stdin.take().expect("stdin is piped");
    let shas: String = files.iter().map(|(sha, _)| format!("{sha}\n")).collect();
    // Written from another thread so a large answer cannot block the request.
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        let _ = stdin.write_all(shas.as_bytes());
    });
    let mut out = Vec::new();
    {
        use std::io::Read;
        cat.stdout.take().expect("stdout is piped").read_to_end(&mut out).map_err(|e| format!("cannot read from git: {e}"))?;
    }
    let _ = writer.join();
    let _ = cat.wait();

    let mut at = 0;
    for (_, path) in &files {
        let Some(nl) = out[at..].iter().position(|b| *b == b'\n') else { return Err("git cat-file ended early".into()) };
        let header = String::from_utf8_lossy(&out[at..at + nl]).to_string();
        at += nl + 1;
        let size: usize = header.rsplit(' ').next().and_then(|n| n.parse().ok()).ok_or_else(|| format!("unexpected answer from git: {header}"))?;
        let body = out.get(at..at + size).ok_or("git cat-file ended early")?;
        at += size + 1;
        let target = dir.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(&target, body).map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    }
    Ok(())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, ()> {
    let out = Command::new("git").current_dir(cwd).args(args).stderr(Stdio::null()).output().map_err(|_| ())?;
    if !out.status.success() {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
