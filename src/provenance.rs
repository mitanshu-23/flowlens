//! Tells user-written code apart from the standard library and dependencies.
//!
//! This is the bifurcation the rest of the tool hangs off: a call to
//! `normalize()` in your own module is a fact about *your* design, while a call
//! to `.trim()` or `HashMap::new()` is noise you did not write and cannot
//! change. They belong in different lists.
//!
//! Classification is by the head segment of the path a name came in through:
//! `crate::db::connect` is yours, `serde_json::Value` is a declared dependency,
//! `std::collections::HashMap` is the standard library.

use std::collections::BTreeSet;
use std::path::Path;

/// Crates always in scope without appearing in `Cargo.toml`.
const SYSTEM_CRATES: &[&str] = &["std", "core", "alloc", "proc_macro", "test"];

/// Macros the language ships. They are never imported, so there is no `use` to
/// classify them by, but attributing them to the user would be wrong.
const STD_MACROS: &[&str] = &[
    "println",
    "print",
    "eprintln",
    "eprint",
    "format",
    "format_args",
    "write",
    "writeln",
    "vec",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
    "todo",
    "unimplemented",
    "unreachable",
    "dbg",
    "include",
    "include_str",
    "include_bytes",
    "env",
    "option_env",
    "concat",
    "stringify",
    "line",
    "file",
    "column",
    "module_path",
    "cfg",
    "compile_error",
    "try",
];

/// Where a name a file refers to actually comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Provenance {
    /// Defined in the file under analysis.
    Local,
    /// Defined elsewhere in this crate — still code the user wrote.
    FirstParty,
    /// The standard library, or a crate listed in `Cargo.toml`.
    ThirdParty,
    /// Undetermined. Almost always a method call whose receiver type is not
    /// inferred, so there is no path to classify.
    Unknown,
}

impl Provenance {
    /// Did the user write the thing being referred to?
    pub fn is_user_code(&self) -> bool {
        matches!(self, Provenance::Local | Provenance::FirstParty)
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Provenance::Local => "local",
            Provenance::FirstParty => "crate",
            Provenance::ThirdParty => "library",
            Provenance::Unknown => "unknown",
        }
    }
}

/// What the analyzer knows about the crate being read: its name, its declared
/// dependencies, and the modules it defines.
#[derive(Debug, Default, Clone)]
pub struct CrateInfo {
    pub name: Option<String>,
    /// Dependency names with `-` normalised to `_`, as they appear in paths.
    pub dependencies: BTreeSet<String>,
    /// Top-level module names of this crate, so `db::connect` (no `crate::`
    /// prefix) is still recognised as first-party.
    pub modules: BTreeSet<String>,
}

impl CrateInfo {
    /// Nothing known — every non-local name lands in `Unknown` unless it is
    /// obviously `std` or an explicit `crate::` path.
    pub fn unknown() -> Self {
        CrateInfo::default()
    }

    /// Read `Cargo.toml`, walking up from `start` until one is found.
    pub fn discover(start: &Path) -> Self {
        let mut dir = if start.is_dir() { Some(start.to_path_buf()) } else { start.parent().map(Path::to_path_buf) };
        while let Some(d) = dir {
            let manifest = d.join("Cargo.toml");
            if manifest.is_file() {
                if let Ok(text) = std::fs::read_to_string(&manifest) {
                    return Self::from_manifest(&text);
                }
            }
            dir = d.parent().map(Path::to_path_buf);
        }
        CrateInfo::unknown()
    }

    /// Minimal `Cargo.toml` reader: enough to learn the package name and which
    /// crate names are dependencies. Avoids pulling in a TOML parser for what
    /// amounts to reading a list of keys.
    pub fn from_manifest(text: &str) -> Self {
        let mut info = CrateInfo::default();
        let mut section = String::new();

        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = head.trim().to_string();
                // `[dependencies.foo]` names the dependency in the header.
                if let Some(dep) = dependency_subsection(&section) {
                    info.dependencies.insert(normalize(dep));
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().trim_matches('"');

            if section == "package" && key == "name" {
                info.name = Some(normalize(value.trim().trim_matches('"')));
            } else if is_dependency_section(&section) {
                info.dependencies.insert(normalize(key));
            }
        }
        info
    }

    /// Classify a name by the path it was reached through.
    ///
    /// `origin` is the `use` the head segment resolved to, when there was one;
    /// `path` is the text at the call site. Passing `None` for both — a bare
    /// `.method()` — yields `Unknown` by design rather than a guess.
    pub fn classify(&self, origin: Option<&str>, path: &str, is_macro: bool) -> Provenance {
        if is_macro {
            let name = path.rsplit("::").next().unwrap_or(path);
            if STD_MACROS.contains(&name) {
                return Provenance::ThirdParty;
            }
        }

        // The prelude is std, and a glob guess says nothing reliable.
        match origin {
            Some("std prelude") => return Provenance::ThirdParty,
            Some(o) if o.contains("(via glob)") => {
                // Fall through to the path itself; a glob is only a hint.
                let head = head_of(o.trim_end_matches("::*  (via glob)"));
                return self.classify_head(head);
            }
            _ => {}
        }

        // `self` alone is the receiver keyword and `crate` alone names nothing;
        // only as the first segment of a longer path do they mean a module.
        let qualified = match origin {
            Some(o) => o.contains("::"),
            None => path.contains("::"),
        };
        let head = origin.map(head_of).unwrap_or_else(|| head_of(path));
        if !qualified && matches!(head, "crate" | "self" | "super" | "Self") {
            return Provenance::Unknown;
        }
        self.classify_head(head)
    }

    fn classify_head(&self, head: &str) -> Provenance {
        if head.is_empty() {
            return Provenance::Unknown;
        }
        // An explicit in-crate path is unambiguous.
        if matches!(head, "crate" | "self" | "super") {
            return Provenance::FirstParty;
        }
        if SYSTEM_CRATES.contains(&head) {
            return Provenance::ThirdParty;
        }
        let head_norm = normalize(head);
        if self.dependencies.contains(&head_norm) {
            return Provenance::ThirdParty;
        }
        // Referring to your own crate by name, e.g. `flowlens::model`.
        if self.name.as_deref() == Some(head_norm.as_str()) {
            return Provenance::FirstParty;
        }
        if self.modules.contains(head) {
            return Provenance::FirstParty;
        }
        Provenance::Unknown
    }
}

/// True for a table whose *keys* are dependency names: `[dependencies]` and
/// the `[target.'cfg(..)'.dependencies]` forms.
///
/// Deliberately false for `[dependencies.tokio]`, whose keys are that one
/// dependency's properties — `version`, `features` — and not crates.
fn is_dependency_section(section: &str) -> bool {
    let last = section.rsplit('.').next().unwrap_or(section);
    matches!(last, "dependencies" | "dev-dependencies" | "build-dependencies")
}

/// `dependencies.serde` -> `serde`, including the `target.*` forms.
fn dependency_subsection(section: &str) -> Option<&str> {
    let (head, name) = section.rsplit_once('.')?;
    let base = head.rsplit_once('.').map(|(_, b)| b).unwrap_or(head);
    matches!(base, "dependencies" | "dev-dependencies" | "build-dependencies").then_some(name)
}

fn head_of(path: &str) -> &str {
    path.split("::").next().unwrap_or(path).trim()
}

fn normalize(name: &str) -> String {
    name.trim().replace('-', "_")
}
