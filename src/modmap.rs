//! Finds the files of a crate by following its `mod` declarations.
//!
//! Globbing `**/*.rs` would over-collect: files behind a `#[cfg]`, files no
//! longer wired in, files belonging to a different target. Walking `mod` from
//! the crate root gives exactly the module tree the compiler sees, and the
//! module path of each file — which is what cross-file resolution needs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ra_ap_syntax::{AstNode, Edition, SourceFile, ast, ast::HasAttrs, ast::HasName};

#[derive(Debug, Clone)]
pub struct ModuleFile {
    pub path: PathBuf,
    /// Module path within the crate; empty for the crate root.
    pub module_path: Vec<String>,
}

impl ModuleFile {
}

/// The files of one crate, in discovery order with the root first.
#[derive(Debug, Default)]
pub struct ModuleMap {
    pub files: Vec<ModuleFile>,
    /// Declarations that named a module with no file behind it.
    pub missing: Vec<(Vec<String>, String)>,
}

/// Crate roots to try, in order, under `dir`.
const ROOT_CANDIDATES: &[&str] = &["src/lib.rs", "src/main.rs", "lib.rs", "main.rs"];

/// Discover the module tree.
///
/// `start` may be a crate directory or a single file. Given a file that is not
/// a crate root, the crate root is looked for alongside it so the file's own
/// module path is known; failing that the file is treated as its own root.
pub fn discover(start: &Path) -> ModuleMap {
    let mut map = ModuleMap::default();

    // A package can hold several crate roots: `src/lib.rs` plus one or more
    // binaries. They are separate crates to rustc, but someone analyzing a repo
    // wants all of their code, so seed from each root that exists. The library
    // root is placed first so it owns the module namespace on any collision.
    let mut roots: Vec<PathBuf> = Vec::new();
    if start.is_dir() {
        for candidate in ROOT_CANDIDATES {
            let p = start.join(candidate);
            if p.is_file() {
                roots.push(p);
            }
        }
    } else if start.is_file() {
        match crate_root_near(start) {
            Some(r) => {
                // Include sibling roots too, so a file reachable only from the
                // binary is still found.
                if let Some(dir) = r.parent() {
                    for candidate in ["lib.rs", "main.rs"] {
                        let p = dir.join(candidate);
                        if p.is_file() {
                            roots.push(p);
                        }
                    }
                }
                if roots.is_empty() {
                    roots.push(r);
                }
            }
            None => {
                map.files.push(ModuleFile { path: start.to_path_buf(), module_path: Vec::new() });
                return map;
            }
        }
    }

    if roots.is_empty() {
        return map;
    }

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue: Vec<ModuleFile> = roots.into_iter().rev().map(|path| ModuleFile { path, module_path: Vec::new() }).collect();

    while let Some(current) = queue.pop() {
        let canonical = current.path.canonicalize().unwrap_or_else(|_| current.path.clone());
        if !seen.insert(canonical) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&current.path) else {
            continue;
        };

        for decl in module_declarations(&src) {
            let mut child_path = current.module_path.clone();
            child_path.extend(decl.inline_prefix.iter().cloned());
            child_path.push(decl.name.clone());

            match resolve_module_file(&current, &decl) {
                Some(file) => queue.push(ModuleFile { path: file, module_path: child_path }),
                None => map.missing.push((current.module_path.clone(), decl.name)),
            }
        }
        map.files.push(current);
    }

    map.files.sort_by(|a, b| a.module_path.len().cmp(&b.module_path.len()).then_with(|| a.module_path.cmp(&b.module_path)));
    map
}

/// A `mod foo;` declaration awaiting a file.
struct Decl {
    name: String,
    /// Inline `mod` blocks the declaration sits inside, which shift the
    /// directory it resolves against.
    inline_prefix: Vec<String>,
    /// An explicit `#[path = "..."]` override.
    explicit_path: Option<String>,
}

fn module_declarations(src: &str) -> Vec<Decl> {
    let parsed = SourceFile::parse(src, Edition::Edition2021);
    let root = parsed.syntax_node();
    let mut out = Vec::new();

    for m in root.descendants().filter_map(ast::Module::cast) {
        // A body means the module lives in this file, not another one.
        if m.item_list().is_some() {
            continue;
        }
        let Some(name) = m.name() else { continue };

        let mut inline_prefix: Vec<String> = m.syntax().ancestors().skip(1).filter_map(ast::Module::cast).filter_map(|p| p.name()).map(|n| n.text().to_string()).collect();
        inline_prefix.reverse();

        let explicit_path = m.attrs().find_map(|a| path_attr_value(&a));

        out.push(Decl { name: name.text().to_string(), inline_prefix, explicit_path });
    }
    out
}

/// The string in `#[path = "some/file.rs"]`, if this attribute is one.
fn path_attr_value(attr: &ast::Attr) -> Option<String> {
    let text = attr.syntax().text().to_string();
    let rest = text.split_once("path")?.1;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let inner = rest.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

/// Where a child module's file lives, per Rust's own rules.
///
/// Children of a *root* file (`lib.rs`, `main.rs`, `mod.rs`) sit beside it;
/// children of any other file sit in a directory named after it. An inline
/// `mod` wrapper adds a directory level for each layer.
fn resolve_module_file(parent: &ModuleFile, decl: &Decl) -> Option<PathBuf> {
    let dir = parent.path.parent()?;
    let stem = parent.path.file_stem()?.to_str()?;
    let is_root_like = matches!(stem, "lib" | "main" | "mod");

    let mut base = if is_root_like { dir.to_path_buf() } else { dir.join(stem) };
    for part in &decl.inline_prefix {
        base = base.join(part);
    }

    if let Some(explicit) = &decl.explicit_path {
        let candidate = base.join(explicit);
        return candidate.is_file().then_some(candidate);
    }

    let flat = base.join(format!("{}.rs", decl.name));
    if flat.is_file() {
        return Some(flat);
    }
    let nested = base.join(&decl.name).join("mod.rs");
    nested.is_file().then_some(nested)
}

/// Walk up from a file looking for the crate root that owns it.
fn crate_root_near(file: &Path) -> Option<PathBuf> {
    let mut dir = file.parent();
    while let Some(d) = dir {
        for candidate in ["lib.rs", "main.rs"] {
            let p = d.join(candidate);
            if p.is_file() && p != file {
                return Some(p);
            }
        }
        if d.join("Cargo.toml").is_file() {
            for candidate in ROOT_CANDIDATES {
                let p = d.join(candidate);
                if p.is_file() {
                    return Some(p);
                }
            }
            break;
        }
        dir = d.parent();
    }
    None
}
