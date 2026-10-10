//! Core data model for the code index.
//!
//! Two tables drive everything, mirroring what an IDE keeps:
//!   * `Symbol`    — a definition (where something *is declared*)
//!   * `Reference` — a use site (where something *is mentioned*)
//!
//! A reference carries both ends of the edge: `from` (the enclosing function,
//! i.e. the caller) and `to` (the resolved definition, i.e. the callee). That
//! single pair answers "who calls this?" and "what does this call?" in both
//! directions, and an unresolved `to` marks a candidate cross-file edge.

use std::fmt;

use crate::provenance::Provenance;

pub type SymbolId = usize;
/// Index of a file within a [`crate::workspace::Workspace`].
pub type FileId = usize;

/// A symbol identified across the whole crate, not just within one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalId {
    pub file: FileId,
    pub sym: SymbolId,
}

/// Maps byte offsets to 1-based line/column. Built once per file.
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(src: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        LineIndex { line_starts }
    }

    /// 1-based (line, column). Column counts characters, not bytes, so it
    /// lines up with what an editor shows on non-ASCII lines.
    pub fn line_col(&self, src: &str, offset: usize) -> (usize, usize) {
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let start = self.line_starts[line];
        let col = src[start..offset.min(src.len())].chars().count() + 1;
        (line + 1, col)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    pub line: usize,
    pub col: usize,
    pub offset: usize,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Const,
    Static,
    Module,
}

impl SymbolKind {
    pub fn tag(&self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::Module => "mod",
        }
    }

    /// Only callables take part in the call graph.
    pub fn is_callable(&self) -> bool {
        matches!(self, SymbolKind::Function | SymbolKind::Method)
    }
}

/// Where a method's owner came from. `impl Repo` and `trait Store` both put a
/// method "inside Repo"/"inside Store", but they mean different things: one is
/// a definition, the other a contract with no body of its own to call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnerKind {
    /// `impl Type { .. }` or `impl Trait for Type { .. }`
    Impl,
    /// `trait Name { .. }` — the declaration itself.
    TraitDecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Crate,
    Public,
}

impl Visibility {
    pub fn tag(&self) -> &'static str {
        match self {
            Visibility::Private => "-",
            Visibility::Crate => "~",
            Visibility::Public => "+",
        }
    }
}

/// One parameter of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The binding, e.g. `path` — or `self` for a receiver.
    pub name: String,
    /// The declared type, e.g. `&str`. Empty for a bare `self`.
    pub ty: String,
}

/// A definition site.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    /// Bare name as declared, e.g. `new`.
    pub name: String,
    /// Path within the file, e.g. `cfg::FlowGraph::new`.
    pub qualified: String,
    pub kind: SymbolKind,
    /// The `impl`/`trait` type this belongs to, if any.
    pub owner: Option<String>,
    /// `impl Trait for Type` — the trait being implemented.
    pub implements: Option<String>,
    pub owner_kind: Option<OwnerKind>,
    /// Enclosing `mod` path, empty at file root.
    pub module_path: Vec<String>,
    pub visibility: Visibility,
    pub is_async: bool,
    pub signature: String,
    /// Parameters in declaration order, receiver first when there is one.
    pub params: Vec<Param>,
    /// The return type as written, without the `->`.
    pub ret: Option<String>,
    /// The `///` doc comment, lines joined, prefixes stripped.
    pub doc: Option<String>,
    pub loc: Loc,
    /// Byte range of the whole item, used to find the enclosing definition.
    pub range: (usize, usize),
}

impl Symbol {
    /// The opening sentence of the doc comment — what the thing is *for*.
    ///
    /// Rust convention puts a one-line summary first, then a blank line, then
    /// detail. That first line is what belongs on a graph node; the rest would
    /// not fit and is better read in the source.
    pub fn doc_summary(&self) -> Option<String> {
        let doc = self.doc.as_deref()?;
        // The leading paragraph, unwrapped into one line.
        let para = doc.lines().map(str::trim).take_while(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        let para = para.trim();
        if para.is_empty() {
            return None;
        }
        // Stop at the first sentence break, when there is one.
        let end = para.find(". ").map(|i| i + 1).unwrap_or_else(|| para.len());
        let summary = para[..end].trim();
        (!summary.is_empty()).then(|| summary.to_string())
    }

    /// Whether this definition is documented at all.
    pub fn is_documented(&self) -> bool {
        self.doc.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// `foo()` or `Type::foo()`
    Call,
    /// `receiver.foo()`
    MethodCall,
    /// `foo!(..)`
    MacroCall,
    /// A function named but not called — passed as a value.
    ValueRef,
    /// A type mentioned in a signature or annotation.
    TypeRef,
}

impl RefKind {
    pub fn tag(&self) -> &'static str {
        match self {
            RefKind::Call => "call",
            RefKind::MethodCall => "method",
            RefKind::MacroCall => "macro",
            RefKind::ValueRef => "value",
            RefKind::TypeRef => "type",
        }
    }
}

/// How a reference was tied to a definition — worth surfacing, because a
/// method call resolved by name alone is a guess, not a fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Bound to a definition in the same file.
    Local(SymbolId),
    /// Bound to a definition in another file of this crate.
    CrossFile(GlobalId),
    /// Name matched more than one definition; no type inference to break the tie.
    Ambiguous(Vec<SymbolId>),
    /// Not bound to any definition the analyzer can see. If the head segment
    /// matched a `use`, the import it came from is recorded.
    Unresolved { origin: Option<String>, provenance: Provenance },
}

impl Resolution {
    pub fn provenance(&self) -> Provenance {
        match self {
            Resolution::Local(_) => Provenance::Local,
            // An ambiguous match is still a match against local definitions.
            Resolution::Ambiguous(_) => Provenance::Local,
            Resolution::CrossFile(_) => Provenance::FirstParty,
            Resolution::Unresolved { provenance, .. } => *provenance,
        }
    }

    /// True when the reference points at something the user wrote.
    pub fn is_user_code(&self) -> bool {
        self.provenance().is_user_code()
    }
}

/// A control-flow construct a reference sits inside.
///
/// Source order alone is misleading: a call inside a `for` runs many times, a
/// call inside an `if` may not run at all. Recording the enclosing constructs
/// turns a flat list of call sites into something you can actually read as the
/// shape of the function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlKind {
    Loop,
    While,
    For,
    If,
    Else,
    MatchArm,
    Closure,
}

impl ControlKind {
    pub fn tag(&self) -> &'static str {
        match self {
            ControlKind::Loop => "loop",
            ControlKind::While => "while",
            ControlKind::For => "for",
            ControlKind::If => "if",
            ControlKind::Else => "else",
            ControlKind::MatchArm => "arm",
            ControlKind::Closure => "closure",
        }
    }

    /// Does being inside this mean the call may run more than once?
    pub fn repeats(&self) -> bool {
        matches!(self, ControlKind::Loop | ControlKind::While | ControlKind::For)
    }

    /// Does being inside this mean the call may not run at all?
    pub fn conditional(&self) -> bool {
        matches!(self, ControlKind::If | ControlKind::Else | ControlKind::MatchArm | ControlKind::While)
    }
}

/// One frame of the control-flow stack around a reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFrame {
    pub kind: ControlKind,
    /// The condition, pattern, or header as written — `n < 10`, `f in &files`.
    pub label: String,
    /// Byte offset where the construct starts, so frames can be identified
    /// across the references that share them.
    pub offset: usize,
    pub line: usize,
}

/// A use site.
#[derive(Debug, Clone)]
pub struct Reference {
    /// Bare name at the call site, e.g. `new`.
    pub name: String,
    /// Path exactly as written, e.g. `FlowGraph::new`.
    pub path: String,
    pub kind: RefKind,
    /// Enclosing callable — the caller. `None` at item level (e.g. a `const`).
    pub from: Option<SymbolId>,
    pub to: Resolution,
    /// Argument expressions exactly as written at this call site.
    pub args: Vec<String>,
    /// The variable the result is stored in — the `x` of `let x = foo()`.
    /// `None` when the value is discarded, passed straight on, or matched.
    pub binding: Option<String>,
    /// Enclosing loops, branches and match arms, outermost first.
    pub context: Vec<ControlFrame>,
    pub loc: Loc,
    /// Resolved by matching a unique method name across the crate rather than
    /// by following an import. A good bet, but a bet: if the receiver is a std
    /// type that happens to share the name, the edge is wrong.
    pub by_name: bool,
    /// Found by re-parsing a macro's arguments rather than in the file's own
    /// syntax tree. Macro bodies are unparsed token trees, so these are
    /// recovered best-effort and can misread non-expression syntax.
    pub via_macro: bool,
}

impl Reference {
    /// The definition this points at, when it lives in the same file.
    pub fn target(&self) -> Option<SymbolId> {
        match &self.to {
            Resolution::Local(id) => Some(*id),
            _ => None,
        }
    }

    /// The definition this points at anywhere in the crate.
    pub fn global_target(&self, own_file: FileId) -> Option<GlobalId> {
        match &self.to {
            Resolution::Local(sym) => Some(GlobalId { file: own_file, sym: *sym }),
            Resolution::CrossFile(g) => Some(*g),
            _ => None,
        }
    }

    pub fn provenance(&self) -> Provenance {
        self.to.provenance()
    }

    /// Does this call sit inside a loop, so it may run many times?
    pub fn in_loop(&self) -> bool {
        self.context.iter().any(|c| c.kind.repeats())
    }

    /// Does this call sit inside a branch, so it may not run at all?
    pub fn is_conditional(&self) -> bool {
        self.context.iter().any(|c| c.kind.conditional())
    }

    /// Is this a call, as opposed to a type or value mention?
    pub fn is_call(&self) -> bool {
        matches!(self.kind, RefKind::Call | RefKind::MethodCall | RefKind::MacroCall)
    }
}
