//! The steps a function takes, in the order it takes them.
//!
//! A step is something a person would mention when describing what the
//! function does: it calls another of *your* functions, it branches, it loops,
//! or it stops — returns early, fails, crashes. Calls into `std` and
//! dependencies are not steps; they are how a step is carried out.
//!
//! Exits are the reason this is a separate walk rather than a reuse of the
//! reference list. `if !path.exists() { return Err(..) }` contains no call of
//! the user's at all, yet it is exactly the line that decides what happens on
//! bad input — and the line an AI edit most often changes.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, TextRange, ast, ast::HasArgList, ast::HasAttrs, ast::HasLoopBody, ast::HasName};

use crate::model::*;
use crate::workspace::Workspace;

/// Longest code fragment kept on a step, in characters.
const MAX_DETAIL: usize = 60;

/// Longest subject kept for a pattern; the page shortens it further.
const MAX_SUBJECT: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    If,
    ElseIf,
    Else,
    /// A `match` arm.
    When,
    ForEach,
    While,
    Repeat,
    /// The `else` of `let PAT = EXPR else { .. }` — runs when the pattern
    /// fails. Also an `if let` whose own branch has no steps but whose `else`
    /// does: the two read the same way.
    LetElse,
    /// An `if` whose own branch has no steps but whose `else` does: the else,
    /// said as "if the condition does not hold".
    Unless,
}

impl BlockKind {
    pub fn tag(&self) -> &'static str {
        match self {
            BlockKind::If => "if",
            BlockKind::ElseIf => "elseif",
            BlockKind::Else => "else",
            BlockKind::When => "when",
            BlockKind::ForEach => "for",
            BlockKind::While => "while",
            BlockKind::Repeat => "loop",
            BlockKind::LetElse => "letelse",
            BlockKind::Unless => "unless",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    /// `return x`
    Return,
    /// `return Err(..)`, or `?` on something that is not one of the user's calls.
    Fail,
    /// `panic!`, `unreachable!`, `assert!`, `.unwrap()`, `.expect()`.
    Crash,
    /// `todo!()` and `unimplemented!()`.
    Unfinished,
    /// `std::process::exit`.
    ExitProgram,
    Break,
    Continue,
}

impl ExitKind {
    pub fn tag(&self) -> &'static str {
        match self {
            ExitKind::Return => "return",
            ExitKind::Fail => "fail",
            ExitKind::Crash => "crash",
            ExitKind::Unfinished => "todo",
            ExitKind::ExitProgram => "exit",
            ExitKind::Break => "break",
            ExitKind::Continue => "continue",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A call to one of the user's own functions.
    Call {
        /// Stable key of the callee, see [`function_keys`].
        target: String,
        args: Vec<String>,
        /// Followed by `?`: the function stops here if this call fails.
        fallible: bool,
        /// Followed by `.unwrap()` or `.expect()`: the program crashes if it fails.
        crashes: bool,
        line: usize,
        /// Byte offset of the call, to attach a later `?` to it. Not compared.
        offset: usize,
    },
    /// A value set that a later decision reads: `all_valid = false`,
    /// `retry_count -= 1`. Only kept for variables a decision depends on and
    /// the function changes along the way — flags and counters.
    Set {
        var: String,
        /// `let` for where it starts, else the operator: `=`, `-=`, `+=`, …
        op: String,
        value: String,
        line: usize,
    },
    Exit {
        kind: ExitKind,
        /// The code that decides the exit, e.g. the returned value. For
        /// `break` and `continue`, which loop they act on, counted outwards
        /// from the innermost one enclosing them: `0`, or `1` for the loop
        /// around that — what `break 'outer` resolves to.
        detail: String,
        line: usize,
    },
    Block {
        kind: BlockKind,
        /// The condition, pattern, or loop header as written. For a match arm,
        /// `if let` or `let … else` this is the pattern alone, and `on` holds
        /// what it is matched against.
        header: String,
        /// What a pattern is checked against, as written: `data.get("id")`.
        /// Empty for a plain condition or a loop.
        on: String,
        /// When what is checked is the value of one of the user's own calls,
        /// that function's key — so the page can say "if reading the file
        /// fails" rather than quote the code.
        on_call: Option<String>,
        line: usize,
        steps: Vec<Step>,
    },
}

/// Everything known about one function's body.
#[derive(Debug, Clone)]
pub struct Flow {
    pub steps: Vec<Step>,
    /// The body's tokens with comments and whitespace removed. Two versions
    /// with the same fingerprint behave identically.
    pub fingerprint: String,
    /// The function's source text, from its first line.
    pub source: String,
    pub first_line: usize,
    /// A test, not part of the program the user runs.
    pub is_test: bool,
    /// The variables it declares, with where they start and change.
    pub vars: Vec<Var>,
}

/// A local variable: `let mut all_valid = true;` and every later
/// `all_valid = false;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Var {
    pub name: String,
    pub mutable: bool,
    /// The type as written, when it is written.
    pub ty: Option<String>,
    /// What it starts as; `None` for `let x;`.
    pub init: Option<String>,
    pub line: usize,
    /// Each later assignment: line, operator (`=`, `+=` …), value.
    pub sets: Vec<(usize, String, String)>,
}

/// A name for every callable that stays the same across two versions of the
/// crate: `crate::workspace::Workspace::load`. Duplicates get a `#n` suffix.
///
/// `prefix` replaces the leading `crate`, so functions from several crates of
/// one workspace can share a map: `api::main` and `worker::main`.
pub fn function_keys(ws: &Workspace, prefix: &str) -> HashMap<GlobalId, String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = HashMap::new();
    for (fid, f) in ws.files.iter().enumerate() {
        for s in f.callables() {
            let g = GlobalId { file: fid, sym: s.id };
            let full = ws.full_name(g);
            let base = match full.strip_prefix("crate") {
                Some(rest) => format!("{prefix}{rest}"),
                None => full,
            };
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            let key = if *n == 1 { base } else { format!("{base}#{n}") };
            out.insert(g, key);
        }
    }
    out
}

/// The flow of every callable in the workspace, by key.
pub fn flows(ws: &Workspace, keys: &HashMap<GlobalId, String>) -> HashMap<String, Flow> {
    let mut out = HashMap::new();
    for (fid, f) in ws.files.iter().enumerate() {
        let root = SourceFile::parse(&f.src, Edition::Edition2021).syntax_node();
        let fns: HashMap<usize, ast::Fn> = root.descendants().filter_map(ast::Fn::cast).map(|func| (usize::from(func.syntax().text_range().start()), func)).collect();

        // Calls by where they sit, so the walk can find what each resolved to.
        let mut calls: HashMap<(usize, &str), &Reference> = HashMap::new();
        for r in f.references.iter().filter(|r| r.is_call() && !r.via_macro) {
            calls.insert((r.loc.offset, r.name.as_str()), r);
        }

        for s in f.callables() {
            let Some(func) = fns.get(&s.range.0) else { continue };
            let g = GlobalId { file: fid, sym: s.id };
            let mut tails = HashSet::new();
            if let Some(body) = func.body() {
                mark_tails(&ast::Expr::BlockExpr(body), &mut tails);
            }
            let tracked = func.body().map(|b| decision_state(b.syntax())).unwrap_or_default();
            let walker = Walker { ws, fid, sym: s.id, keys, calls: &calls, lines: &f.lines, src: &f.src, tails, tracked, loops: RefCell::new(Vec::new()) };
            let mut steps = Vec::new();
            if let Some(body) = func.body() {
                walker.walk(body.syntax(), &mut steps);
            }
            out.insert(
                keys[&g].clone(),
                Flow {
                    steps,
                    fingerprint: fingerprint(func.syntax()),
                    source: f.src[s.range.0..s.range.1].to_string(),
                    first_line: f.lines.line_col(&f.src, s.range.0).0,
                    is_test: is_test(func),
                    vars: func.body().map(|b| variables(b.syntax(), &f.lines, &f.src)).unwrap_or_default(),
                },
            );
        }
    }
    out
}

/// Is this a test rather than part of the program? `#[test]`, `#[tokio::test]`
/// and friends, or anything inside a `#[cfg(test)]` module.
fn is_test(func: &ast::Fn) -> bool {
    let test_attr = func.attrs().any(|a| a.path().and_then(|p| p.segment()).and_then(|s| s.name_ref()).is_some_and(|n| n.text() == "test"));
    test_attr
        || func.syntax().ancestors().filter_map(ast::Module::cast).any(|m| {
            m.attrs().any(|a| {
                let t = a.syntax().text().to_string();
                t.contains("cfg") && t.contains("test")
            })
        })
}

// ------------------------------------------------------------------ walk

struct Walker<'a> {
    ws: &'a Workspace,
    fid: FileId,
    sym: SymbolId,
    keys: &'a HashMap<GlobalId, String>,
    calls: &'a HashMap<(usize, &'a str), &'a Reference>,
    lines: &'a LineIndex,
    src: &'a str,
    /// Expressions whose value is what the function returns.
    tails: HashSet<TextRange>,
    /// Variables whose assignments are steps; see [`decision_state`].
    tracked: HashSet<String>,
    /// The loops around the node being walked, innermost last, with their
    /// labels — so a `break 'outer` can say which loop it leaves.
    loops: RefCell<Vec<Option<String>>>,
}

impl Walker<'_> {
    fn line(&self, node: &SyntaxNode) -> usize {
        self.lines.line_col(self.src, node.text_range().start().into()).0
    }

    /// Steps of `node`, appended to `out` in the order they would run.
    fn walk(&self, node: &SyntaxNode, out: &mut Vec<Step>) {
        // A nested `fn` or `impl` is its own definition, not part of this body.
        if ast::Item::can_cast(node.kind()) && !ast::MacroCall::can_cast(node.kind()) {
            return;
        }
        if let Some(e) = ast::IfExpr::cast(node.clone()) {
            return self.if_chain(&e, BlockKind::If, out);
        }
        if let Some(m) = ast::MatchExpr::cast(node.clone()) {
            let mut on = Subject::default();
            if let Some(scrutinee) = m.expr() {
                self.walk(scrutinee.syntax(), out);
                on = self.subject(&scrutinee, out);
            }
            for arm in m.match_arm_list().into_iter().flat_map(|l| l.arms()) {
                let mut header = arm.pat().map(|p| text(p.syntax())).unwrap_or_default();
                let mut steps = Vec::new();
                if let Some(guard) = arm.guard() {
                    if let Some(cond) = guard.condition() {
                        header = format!("{header} if {}", text(cond.syntax()));
                    }
                    self.walk(guard.syntax(), &mut steps);
                }
                if let Some(e) = arm.expr() {
                    self.walk(e.syntax(), &mut steps);
                }
                push_block(out, BlockKind::When, header, on.clone(), self.line(arm.syntax()), steps);
            }
            return;
        }
        if let Some(e) = ast::ForExpr::cast(node.clone()) {
            if let Some(iter) = e.iterable() {
                self.walk(iter.syntax(), out);
            }
            let header = format!("{} in {}", e.pat().map(|p| text(p.syntax())).unwrap_or_default(), e.iterable().map(|i| text(i.syntax())).unwrap_or_default());
            let mut steps = Vec::new();
            self.in_loop(e.label(), || {
                if let Some(body) = e.loop_body() {
                    self.walk(body.syntax(), &mut steps);
                }
            });
            push_block(out, BlockKind::ForEach, header, Subject::default(), self.line(node), steps);
            return;
        }
        if let Some(e) = ast::WhileExpr::cast(node.clone()) {
            let header = e.condition().map(|c| text(c.syntax())).unwrap_or_default();
            let mut steps = Vec::new();
            self.in_loop(e.label(), || {
                if let Some(c) = e.condition() {
                    self.walk(c.syntax(), &mut steps);
                }
                if let Some(body) = e.loop_body() {
                    self.walk(body.syntax(), &mut steps);
                }
            });
            push_block(out, BlockKind::While, header, Subject::default(), self.line(node), steps);
            return;
        }
        if let Some(e) = ast::LoopExpr::cast(node.clone()) {
            let mut steps = Vec::new();
            self.in_loop(e.label(), || {
                if let Some(body) = e.loop_body() {
                    self.walk(body.syntax(), &mut steps);
                }
            });
            push_block(out, BlockKind::Repeat, String::new(), Subject::default(), self.line(node), steps);
            return;
        }
        if let Some(stmt) = ast::LetStmt::cast(node.clone()) {
            if let (None, Some(ast::Pat::IdentPat(ip)), Some(init)) = (stmt.let_else(), stmt.pat(), stmt.initializer()) {
                let var = ip.name().map(|n| n.text().to_string()).unwrap_or_default();
                if self.tracked.contains(&var) {
                    self.walk(init.syntax(), out);
                    out.push(Step::Set { var, op: "let".into(), value: text(init.syntax()), line: self.line(node) });
                    return;
                }
            }
            if let Some(else_) = stmt.let_else() {
                let mut on = Subject::default();
                if let Some(init) = stmt.initializer() {
                    self.walk(init.syntax(), out);
                    on = self.subject(&init, out);
                }
                let header = stmt.pat().map(|p| text(p.syntax())).unwrap_or_default();
                let mut steps = Vec::new();
                if let Some(block) = else_.block_expr() {
                    self.walk(block.syntax(), &mut steps);
                }
                push_block(out, BlockKind::LetElse, header, on, self.line(node), steps);
                return;
            }
        }
        if let Some(bin) = ast::BinExpr::cast(node.clone()) {
            if let (Some(op), Some(var), Some(rhs)) = (assignment_op(&bin), bin.lhs().and_then(|l| local_name(&l)), bin.rhs()) {
                if self.tracked.contains(&var) {
                    self.walk(rhs.syntax(), out);
                    out.push(Step::Set { var, op, value: text(rhs.syntax()), line: self.line(node) });
                    return;
                }
            }
        }
        if let Some(r) = ast::ReturnExpr::cast(node.clone()) {
            let value = r.expr();
            if let Some(v) = &value {
                self.walk(v.syntax(), out);
            }
            let detail = value.as_ref().map(|v| whole(v.syntax())).unwrap_or_default();
            let kind = if detail.starts_with("Err(") || detail.starts_with("Err (") { ExitKind::Fail } else { ExitKind::Return };
            out.push(Step::Exit { kind, detail, line: self.line(node) });
            return;
        }
        if let Some(t) = ast::TryExpr::cast(node.clone()) {
            let Some(inner) = t.expr() else { return };
            self.walk(inner.syntax(), out);
            if !self.mark_last_call(&inner, out, |fallible, _| *fallible = true) {
                out.push(Step::Exit { kind: ExitKind::Fail, detail: whole(inner.syntax()), line: self.line(node) });
            }
            return;
        }
        if ast::BreakExpr::can_cast(node.kind()) || ast::ContinueExpr::can_cast(node.kind()) {
            let kind = if node.kind() == SyntaxKind::BREAK_EXPR { ExitKind::Break } else { ExitKind::Continue };
            // `break value` evaluates its value first.
            for child in node.children() {
                self.walk(&child, out);
            }
            let label = ast::BreakExpr::cast(node.clone()).and_then(|b| b.lifetime()).or_else(|| ast::ContinueExpr::cast(node.clone()).and_then(|c| c.lifetime())).map(|l| l.syntax().text().to_string());
            let up = {
                let loops = self.loops.borrow();
                match &label {
                    Some(l) => loops.iter().rev().position(|x| x.as_deref() == Some(l.as_str())).unwrap_or(0),
                    None => 0,
                }
            };
            out.push(Step::Exit { kind, detail: up.to_string(), line: self.line(node) });
            return;
        }
        if ast::ClosureExpr::can_cast(node.kind()) {
            // A `break` cannot reach out of a closure: its loops are its own.
            let outer = self.loops.replace(Vec::new());
            for child in node.children() {
                self.walk(&child, out);
            }
            self.loops.replace(outer);
            return;
        }
        if let Some(mac) = ast::MacroCall::cast(node.clone()) {
            return self.macro_call(&mac, out);
        }
        if let Some(call) = ast::CallExpr::cast(node.clone()) {
            if let Some(args) = call.arg_list() {
                self.walk(args.syntax(), out);
            }
            let Some(ast::Expr::PathExpr(callee)) = call.expr() else { return };
            let path = text(callee.syntax());
            if path == "process::exit" || path.ends_with("::process::exit") {
                out.push(Step::Exit { kind: ExitKind::ExitProgram, detail: call.arg_list().map(|a| text(a.syntax())).unwrap_or_default(), line: self.line(node) });
                return;
            }
            // `Err(..)` as the function's last expression is a `return Err(..)`
            // in all but spelling.
            if path == "Err" && self.tails.contains(&node.text_range()) {
                out.push(Step::Exit { kind: ExitKind::Fail, detail: whole(node), line: self.line(node) });
                return;
            }
            // The name from the path's last segment, not its text: with type
            // arguments, `get::<String>` ends in `<String>`.
            let name = callee.path().and_then(|p| p.segment()).and_then(|s| s.name_ref()).map(|n| n.text().to_string()).unwrap_or_else(|| path.rsplit("::").next().unwrap_or(&path).trim().to_string());
            self.push_call(node, &name, out);
            return;
        }
        if let Some(mc) = ast::MethodCallExpr::cast(node.clone()) {
            if let Some(recv) = mc.receiver() {
                self.walk(recv.syntax(), out);
            }
            if let Some(args) = mc.arg_list() {
                self.walk(args.syntax(), out);
            }
            let Some(name) = mc.name_ref().map(|n| n.text().to_string()) else { return };
            if name == "unwrap" || name == "expect" {
                let Some(recv) = mc.receiver() else { return };
                if !self.mark_last_call(&recv, out, |_, crashes| *crashes = true) {
                    out.push(Step::Exit { kind: ExitKind::Crash, detail: format!("{}.{name}()", text(recv.syntax())), line: self.line(node) });
                }
                return;
            }
            self.push_call(node, &name, out);
            return;
        }
        for child in node.children() {
            self.walk(&child, out);
        }
    }

    /// Walk a loop's body with the loop on the stack.
    fn in_loop(&self, label: Option<ast::Label>, body: impl FnOnce()) {
        let name = label.and_then(|l| l.lifetime()).map(|l| l.syntax().text().to_string());
        self.loops.borrow_mut().push(name);
        body();
        self.loops.borrow_mut().pop();
    }

    /// `if a {..} else if b {..} else {..}` as sibling blocks, not nested ones:
    /// that is how a person reads it.
    fn if_chain(&self, e: &ast::IfExpr, kind: BlockKind, out: &mut Vec<Step>) {
        let cond = e.condition();
        if let Some(c) = &cond {
            self.walk(c.syntax(), out);
        }
        let mut steps = Vec::new();
        if let Some(then) = e.then_branch() {
            self.walk(then.syntax(), &mut steps);
        }
        // `if let PAT = EXPR` checks a pattern against a value, like a match arm.
        let (header, on) = match &cond {
            Some(ast::Expr::LetExpr(l)) => (l.pat().map(|p| text(p.syntax())).unwrap_or_default(), l.expr().map(|x| self.subject(&x, out)).unwrap_or_default()),
            Some(c) => (text(c.syntax()), Subject::default()),
            None => (String::new(), Subject::default()),
        };
        let is_let = matches!(cond, Some(ast::Expr::LetExpr(_)));
        let line = self.line(e.syntax());

        match e.else_branch() {
            // Nothing to show when it holds, something when it does not: one
            // step, the condition turned round, rather than an "otherwise"
            // with nothing before it to be otherwise to.
            Some(ast::ElseBranch::Block(b)) if steps.is_empty() && kind == BlockKind::If => {
                let mut other = Vec::new();
                self.walk(b.syntax(), &mut other);
                let turned = if is_let { BlockKind::LetElse } else { BlockKind::Unless };
                push_block(out, turned, header, on, line, other);
            }
            Some(ast::ElseBranch::IfExpr(next)) => {
                // Kept even when empty, so the branches after it can say what
                // they are the alternative to.
                let mut rest = Vec::new();
                self.if_chain(&next, BlockKind::ElseIf, &mut rest);
                if !rest.is_empty() {
                    out.push(block(kind, header, on, line, steps));
                    out.extend(rest);
                } else {
                    push_block(out, kind, header, on, line, steps);
                }
            }
            Some(ast::ElseBranch::Block(b)) => {
                let mut other = Vec::new();
                self.walk(b.syntax(), &mut other);
                if other.is_empty() {
                    push_block(out, kind, header, on, line, steps);
                } else {
                    out.push(block(kind, header, on, line, steps));
                    push_block(out, BlockKind::Else, String::new(), Subject::default(), self.line(b.syntax()), other);
                }
            }
            None => push_block(out, kind, header, on, line, steps),
        }
    }

    /// Macros: some are exits in disguise, the rest may hide calls in their
    /// arguments, which the indexer already recovered.
    fn macro_call(&self, mac: &ast::MacroCall, out: &mut Vec<Step>) {
        let name = mac.path().and_then(|p| p.segment()).and_then(|s| s.name_ref()).map(|n| n.text().to_string()).unwrap_or_default();
        let args = mac
            .token_tree()
            .map(|t| {
                let t = text(t.syntax());
                t.get(1..t.len().saturating_sub(1)).unwrap_or("").trim().to_string()
            })
            .unwrap_or_default();
        let line = self.line(mac.syntax());

        // Calls inside the arguments run before the macro itself.
        let range = mac.syntax().text_range();
        let (start, end): (usize, usize) = (range.start().into(), range.end().into());
        let mut inside: Vec<&Reference> = self.ws.files[self.fid].references.iter().filter(|r| r.via_macro && r.is_call() && r.from == Some(self.sym) && r.loc.offset >= start && r.loc.offset < end).collect();
        inside.sort_by_key(|r| r.loc.offset);
        for r in inside {
            if let Some(target) = self.user_target(r) {
                out.push(Step::Call { target, args: r.args.clone(), fallible: false, crashes: false, line: r.loc.line, offset: r.loc.offset });
            }
        }

        let kind = match name.as_str() {
            "panic" | "unreachable" | "assert" | "assert_eq" | "assert_ne" => ExitKind::Crash,
            "todo" | "unimplemented" => ExitKind::Unfinished,
            _ => return,
        };
        let detail = if name.starts_with("assert") { format!("{name}!({args})") } else { args };
        out.push(Step::Exit { kind, detail: clip(&detail), line });
    }

    fn push_call(&self, node: &SyntaxNode, name: &str, out: &mut Vec<Step>) {
        let offset: usize = node.text_range().start().into();
        let Some(r) = self.calls.get(&(offset, name)) else { return };
        let Some(target) = self.user_target(r) else { return };
        out.push(Step::Call { target, args: r.args.clone(), fallible: false, crashes: false, line: r.loc.line, offset });
    }

    /// The key of the user function a reference lands on, if it lands on one.
    fn user_target(&self, r: &Reference) -> Option<String> {
        let g = r.global_target(self.fid)?;
        if !self.ws.symbol(g).kind.is_callable() {
            // `Enum::Variant(..)` and tuple-struct constructors build values;
            // they are not steps.
            return None;
        }
        self.keys.get(&g).cloned()
    }

    /// What a pattern is checked against: the code, and — when it is the value
    /// of the user's own call just recorded — that function.
    fn subject(&self, expr: &ast::Expr, out: &[Step]) -> Subject {
        let mut e = expr.clone();
        while let Some(inner) = match &e {
            ast::Expr::AwaitExpr(a) => a.expr(),
            ast::Expr::ParenExpr(p) => p.expr(),
            ast::Expr::RefExpr(r) => r.expr(),
            _ => None,
        } {
            e = inner;
        }
        let at: usize = e.syntax().text_range().start().into();
        let call = match out.last() {
            Some(Step::Call { offset, target, .. }) if *offset == at && matches!(e, ast::Expr::CallExpr(_) | ast::Expr::MethodCallExpr(_)) => Some(target.clone()),
            _ => None,
        };
        Subject { text: whole(e.syntax()), call }
    }

    /// When `expr` is one of the user's calls — possibly behind `.await` or
    /// parentheses — and it is the step just recorded, apply `f` to its flags.
    fn mark_last_call(&self, expr: &ast::Expr, out: &mut [Step], f: impl FnOnce(&mut bool, &mut bool)) -> bool {
        let mut e = expr.clone();
        while let Some(inner) = match &e {
            ast::Expr::AwaitExpr(a) => a.expr(),
            ast::Expr::ParenExpr(p) => p.expr(),
            _ => None,
        } {
            e = inner;
        }
        let at: usize = e.syntax().text_range().start().into();
        let is_call = matches!(e, ast::Expr::CallExpr(_) | ast::Expr::MethodCallExpr(_));
        match out.last_mut() {
            Some(Step::Call { offset, fallible, crashes, .. }) if is_call && *offset == at => {
                f(fallible, crashes);
                true
            }
            _ => false,
        }
    }
}

/// Record `e` and, through blocks, branches and arms, every expression whose
/// value becomes `e`'s value.
fn mark_tails(e: &ast::Expr, tails: &mut HashSet<TextRange>) {
    tails.insert(e.syntax().text_range());
    match e {
        ast::Expr::BlockExpr(b) => {
            if let Some(t) = b.tail_expr() {
                mark_tails(&t, tails);
            }
        }
        ast::Expr::ParenExpr(p) => {
            if let Some(inner) = p.expr() {
                mark_tails(&inner, tails);
            }
        }
        ast::Expr::IfExpr(i) => {
            if let Some(t) = i.then_branch() {
                mark_tails(&ast::Expr::BlockExpr(t), tails);
            }
            match i.else_branch() {
                Some(ast::ElseBranch::Block(b)) => mark_tails(&ast::Expr::BlockExpr(b), tails),
                Some(ast::ElseBranch::IfExpr(n)) => mark_tails(&ast::Expr::IfExpr(n), tails),
                None => {}
            }
        }
        ast::Expr::MatchExpr(m) => {
            for arm in m.match_arm_list().into_iter().flat_map(|l| l.arms()) {
                if let Some(x) = arm.expr() {
                    mark_tails(&x, tails);
                }
            }
        }
        _ => {}
    }
}

/// Every `let` with a plain name in a body — not in nested functions — and
/// every assignment to it afterwards, in order.
fn variables(body: &SyntaxNode, lines: &LineIndex, src: &str) -> Vec<Var> {
    let line = |n: &SyntaxNode| lines.line_col(src, n.text_range().start().into()).0;
    let own = |n: &SyntaxNode| !n.ancestors().skip(1).take_while(|a| a != body).any(|a| ast::Fn::can_cast(a.kind()));
    let mut vars: Vec<Var> = Vec::new();
    for stmt in body.descendants().filter_map(ast::LetStmt::cast) {
        if !own(stmt.syntax()) {
            continue;
        }
        let Some(ast::Pat::IdentPat(ip)) = stmt.pat() else { continue };
        let Some(name) = ip.name().map(|n| n.text().to_string()) else { continue };
        vars.push(Var { name, mutable: ip.mut_token().is_some(), ty: stmt.ty().map(|t| text(t.syntax())), init: stmt.initializer().map(|i| clip_to(&whole(i.syntax()), 120)), line: line(stmt.syntax()), sets: Vec::new() });
    }
    for bin in body.descendants().filter_map(ast::BinExpr::cast) {
        if !own(bin.syntax()) {
            continue;
        }
        let (Some(op), Some(name), Some(rhs)) = (assignment_op(&bin), bin.lhs().and_then(|l| local_name(&l)), bin.rhs()) else {
            continue;
        };
        // The nearest declaration above it with that name.
        let at = line(bin.syntax());
        if let Some(v) = vars.iter_mut().rev().find(|v| v.name == name && v.line <= at) {
            v.sets.push((at, op, clip_to(&whole(rhs.syntax()), 120)));
        }
    }
    vars
}

/// The variables whose story a reader needs: read by a decision — an `if` or
/// `while` condition, or a match guard — and changed somewhere in the body.
/// `all_valid` and `retry_count` qualify; a value computed once and only
/// passed along does not.
fn decision_state(body: &SyntaxNode) -> HashSet<String> {
    let mut read = HashSet::new();
    let mut note_reads = |cond: Option<ast::Expr>| {
        let Some(cond) = cond else { return };
        if matches!(cond, ast::Expr::LetExpr(_)) {
            return;
        }
        for pe in cond.syntax().descendants().filter_map(ast::PathExpr::cast) {
            if let Some(name) = local_name(&ast::Expr::PathExpr(pe)) {
                read.insert(name);
            }
        }
    };
    for n in body.descendants() {
        if let Some(e) = ast::IfExpr::cast(n.clone()) {
            note_reads(e.condition());
        } else if let Some(e) = ast::WhileExpr::cast(n.clone()) {
            note_reads(e.condition());
        } else if let Some(g) = ast::MatchGuard::cast(n.clone()) {
            note_reads(g.condition());
        }
    }
    body.descendants().filter_map(ast::BinExpr::cast).filter(|b| assignment_op(b).is_some()).filter_map(|b| b.lhs().and_then(|l| local_name(&l))).filter(|name| read.contains(name)).collect()
}

/// `=`, `+=`, `-=` … for an assignment; `None` for any other operator.
fn assignment_op(bin: &ast::BinExpr) -> Option<String> {
    match bin.op_details()? {
        (token, ra_ap_syntax::ast::BinaryOp::Assignment { .. }) => Some(token.text().to_string()),
        _ => None,
    }
}

/// A plain local name — `all_valid` — not a path, a field or a constant.
fn local_name(e: &ast::Expr) -> Option<String> {
    let ast::Expr::PathExpr(pe) = e else { return None };
    let path = pe.path()?;
    if path.qualifier().is_some() {
        return None;
    }
    let name = path.segment()?.name_ref()?.text().to_string();
    let lowercase = name.chars().next().is_some_and(|c| c.is_lowercase() || c == '_');
    (lowercase && name != "self").then_some(name)
}

/// What a pattern is checked against; see [`Step::Block`].
#[derive(Debug, Clone, Default)]
struct Subject {
    text: String,
    call: Option<String>,
}

/// Keep a block only if something happens inside it. An `if` whose body only
/// logs is not a step anyone needs to review.
fn push_block(out: &mut Vec<Step>, kind: BlockKind, header: String, on: Subject, line: usize, steps: Vec<Step>) {
    if steps.is_empty() {
        return;
    }
    out.push(block(kind, header, on, line, steps));
}

fn block(kind: BlockKind, header: String, on: Subject, line: usize, steps: Vec<Step>) -> Step {
    // A pattern is kept whole too, so a guard can be split off it.
    let header = if on.text.is_empty() { clip(&header) } else { clip_to(&header, MAX_SUBJECT) };
    Step::Block { kind, header, on: on.text, on_call: on.call, line, steps }
}

/// A node's text on one line, kept nearly whole: the page trims it by
/// structure — dropping arguments and `Option` plumbing, finding the message
/// in an error — which a cut in the middle would break.
fn whole(node: &SyntaxNode) -> String {
    clip_to(&node.text().to_string().split_whitespace().collect::<Vec<_>>().join(" "), MAX_SUBJECT)
}

/// A node's text on one line, clipped.
fn text(node: &SyntaxNode) -> String {
    clip(&node.text().to_string().split_whitespace().collect::<Vec<_>>().join(" "))
}

fn clip(s: &str) -> String {
    clip_to(s, MAX_DETAIL)
}

fn clip_to(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max - 1).collect();
    format!("{kept}\u{2026}")
}

/// Tokens without comments or whitespace: what the compiler sees.
fn fingerprint(node: &SyntaxNode) -> String {
    node.descendants_with_tokens().filter_map(|e| e.into_token()).filter(|t| t.kind() != SyntaxKind::WHITESPACE && t.kind() != SyntaxKind::COMMENT).map(|t| t.text().to_string()).collect::<Vec<_>>().join(" ")
}
