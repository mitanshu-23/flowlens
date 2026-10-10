//! Getting the "before" version of the code.
//!
//! The reader never sees git; it is only how the old code is fetched. The
//! committed tree at `REF` is exported into a temporary directory with
//! `git archive`, which leaves the user's repository, index and working tree
//! completely untouched — no checkout, no stash, no worktree metadata.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Why there is no "before" to fetch.
#[derive(Debug)]
pub enum Error {
    /// Not a git repository, or one with no commits: there is no history at
    /// all, which is a normal state for a project, not a mistake.
    NoHistory(String),
    /// Anything else: a bad reference, git or tar failing.
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

    let mut archive = Command::new("git").current_dir(&top).args(["archive", "--format=tar", &commit]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| Error::Failed(format!("cannot run git: {e}")))?;
    let stdout = archive.stdout.take().expect("stdout is piped");
    let untar = Command::new("tar").args(["-x", "-C"]).arg(&snap.dir).stdin(stdout).stderr(Stdio::null()).status().map_err(|e| Error::Failed(format!("cannot run tar: {e}")))?;
    let archived = archive.wait().map_err(|e| Error::Failed(format!("git archive failed: {e}")))?;
    if !archived.success() || !untar.success() {
        return Err(Error::Failed(format!("could not export `{reference}` from the repository")));
    }
    Ok(snap)
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, ()> {
    let out = Command::new("git").current_dir(cwd).args(args).stderr(Stdio::null()).output().map_err(|_| ())?;
    if !out.status.success() {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
