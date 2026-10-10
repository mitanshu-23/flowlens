//! Lining up two versions of a function's steps.
//!
//! The alignment is a longest-common-subsequence over each list of sibling
//! steps, the same idea as a text diff but over steps instead of lines. Two
//! steps line up when they are *the same step*: the same function called, the
//! same kind of exit with the same value, the same branch on the same
//! condition. A matched branch is then compared inside, recursively.
//!
//! What is left over is added or removed — except that a removed branch next
//! to an added branch of the same kind is almost always the same branch with
//! an edited condition, and showing it as "changed" keeps its contents from
//! being listed twice.

use super::flow::Step;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Same,
    Added,
    Removed,
    Changed,
}

impl Mark {
    pub fn tag(&self) -> &'static str {
        match self {
            Mark::Same => "same",
            Mark::Added => "add",
            Mark::Removed => "del",
            Mark::Changed => "chg",
        }
    }
}

/// One row of the before/now comparison.
#[derive(Debug, Clone)]
pub struct Row {
    pub mark: Mark,
    /// The step as it was, without its children.
    pub before: Option<Step>,
    /// The step as it is now, without its children.
    pub after: Option<Step>,
    /// For a branch or loop: its contents, compared.
    pub children: Vec<Row>,
}

impl Row {
    /// Does anything in or under this row differ?
    pub fn differs(&self) -> bool {
        self.mark != Mark::Same || self.children.iter().any(Row::differs)
    }
}

/// Compare two step lists.
pub fn diff_steps(before: &[Step], after: &[Step]) -> Vec<Row> {
    let pairs = align(before, after, same_step);
    let mut rows = Vec::new();
    let mut removed: Vec<&Step> = Vec::new();
    let mut added: Vec<&Step> = Vec::new();

    for (b, a) in pairs {
        match (b, a) {
            (Some(b), Some(a)) => {
                flush(&mut removed, &mut added, &mut rows);
                rows.push(matched(&before[b], &after[a]));
            }
            (Some(b), None) => removed.push(&before[b]),
            (None, Some(a)) => added.push(&after[a]),
            (None, None) => {}
        }
    }
    flush(&mut removed, &mut added, &mut rows);
    rows
}

/// Every step of one version, all marked the same way — for a function that
/// only exists on one side.
pub fn one_sided(steps: &[Step], mark: Mark) -> Vec<Row> {
    steps
        .iter()
        .map(|s| {
            let (before, after) = match mark {
                Mark::Removed => (Some(shallow(s)), None),
                Mark::Added => (None, Some(shallow(s))),
                _ => (Some(shallow(s)), Some(shallow(s))),
            };
            Row { mark, before, after, children: one_sided(children(s), mark) }
        })
        .collect()
}

/// A pair of steps that line up.
fn matched(b: &Step, a: &Step) -> Row {
    let mark = match (b, a) {
        (Step::Call { args: ba, fallible: bf, crashes: bc, .. }, Step::Call { args: aa, fallible: af, crashes: ac, .. }) if ba != aa || bf != af || bc != ac => Mark::Changed,
        (Step::Block { header: bh, on: bo, .. }, Step::Block { header: ah, on: ao, .. }) if bh != ah || bo != ao => Mark::Changed,
        (Step::Exit { detail: bd, .. }, Step::Exit { detail: ad, .. }) if bd != ad => Mark::Changed,
        (Step::Set { op: bo, value: bv, .. }, Step::Set { op: ao, value: av, .. }) if bo != ao || bv != av => Mark::Changed,
        _ => Mark::Same,
    };
    Row { mark, before: Some(shallow(b)), after: Some(shallow(a)), children: diff_steps(children(b), children(a)) }
}

/// Emit what was left between two matched steps: pair up same-shaped
/// removed/added steps as changes, then list the rest.
fn flush<'a>(removed: &mut Vec<&'a Step>, added: &mut Vec<&'a Step>, rows: &mut Vec<Row>) {
    let mut used = vec![false; added.len()];
    for b in removed.drain(..) {
        let partner = added.iter().enumerate().find(|(i, a)| !used[*i] && same_shape(b, a)).map(|(i, _)| i);
        match partner {
            Some(i) => {
                used[i] = true;
                let mut row = matched(b, added[i]);
                row.mark = Mark::Changed;
                rows.push(row);
            }
            None => rows.push(Row { mark: Mark::Removed, before: Some(shallow(b)), after: None, children: one_sided(children(b), Mark::Removed) }),
        }
    }
    for (i, a) in added.drain(..).enumerate() {
        if !used[i] {
            rows.push(Row { mark: Mark::Added, before: None, after: Some(shallow(a)), children: one_sided(children(a), Mark::Added) });
        }
    }
}

/// Is this the same step in both versions?
fn same_step(b: &Step, a: &Step) -> bool {
    match (b, a) {
        (Step::Call { target: bt, .. }, Step::Call { target: at, .. }) => bt == at,
        (Step::Exit { kind: bk, detail: bd, .. }, Step::Exit { kind: ak, detail: ad, .. }) => bk == ak && bd == ad,
        (Step::Set { var: bv, op: bo, value: bx, .. }, Step::Set { var: av, op: ao, value: ax, .. }) => bv == av && bo == ao && bx == ax,
        (Step::Block { kind: bk, header: bh, on: bo, .. }, Step::Block { kind: ak, header: ah, on: ao, .. }) => bk == ak && bh == ah && bo == ao,
        _ => false,
    }
}

/// Could this be the same step, edited? Calls never are: a different callee
/// is a different step.
fn same_shape(b: &Step, a: &Step) -> bool {
    match (b, a) {
        (Step::Exit { kind: bk, .. }, Step::Exit { kind: ak, .. }) => bk == ak,
        (Step::Set { var: bv, .. }, Step::Set { var: av, .. }) => bv == av,
        (Step::Block { kind: bk, .. }, Step::Block { kind: ak, .. }) => bk == ak,
        _ => false,
    }
}

fn children(s: &Step) -> &[Step] {
    match s {
        Step::Block { steps, .. } => steps,
        _ => &[],
    }
}

/// The step without its children, which live in [`Row::children`].
fn shallow(s: &Step) -> Step {
    match s {
        Step::Block { kind, header, on, on_call, line, .. } => Step::Block { kind: *kind, header: header.clone(), on: on.clone(), on_call: on_call.clone(), line: *line, steps: Vec::new() },
        other => other.clone(),
    }
}

/// Largest product of lengths aligned exactly; past it, everything on the
/// left counts as removed and everything on the right as added.
const MAX_CELLS: usize = 4_000_000;

/// Longest-common-subsequence alignment of `a` and `b` under `eq`. Returns
/// index pairs in order: `(Some, Some)` for a match, one side `None` otherwise.
pub fn align<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool) -> Vec<(Option<usize>, Option<usize>)> {
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > MAX_CELLS {
        return (0..n).map(|i| (Some(i), None)).chain((0..m).map(|j| (None, Some(j)))).collect();
    }
    // lcs[i][j] = length of the LCS of a[i..] and b[j..].
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if eq(&a[i], &b[j]) { lcs[at(i + 1, j + 1)] + 1 } else { lcs[at(i + 1, j)].max(lcs[at(i, j + 1)]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(n.max(m));
    while i < n && j < m {
        if eq(&a[i], &b[j]) {
            out.push((Some(i), Some(j)));
            i += 1;
            j += 1;
        } else if lcs[at(i + 1, j)] >= lcs[at(i, j + 1)] {
            out.push((Some(i), None));
            i += 1;
        } else {
            out.push((None, Some(j)));
            j += 1;
        }
    }
    out.extend((i..n).map(|i| (Some(i), None)));
    out.extend((j..m).map(|j| (None, Some(j))));
    out
}

/// One line of a source comparison: `(mark, line before, line now, text)`.
pub type CodeLine = (Mark, Option<usize>, Option<usize>, String);

/// A line diff of two versions of a function, for the reader who does want
/// to see the code.
pub fn diff_lines(before: &str, b_first: usize, after: &str, a_first: usize) -> Vec<CodeLine> {
    let b: Vec<&str> = before.lines().collect();
    let a: Vec<&str> = after.lines().collect();
    // Indentation changes alone are not worth a red/green line.
    let norm = |s: &&str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let bn: Vec<String> = b.iter().map(norm).collect();
    let an: Vec<String> = a.iter().map(norm).collect();
    align(&bn, &an, |x, y| x == y)
        .into_iter()
        .map(|(i, j)| match (i, j) {
            (Some(i), Some(j)) => (Mark::Same, Some(b_first + i), Some(a_first + j), a[j].to_string()),
            (Some(i), None) => (Mark::Removed, Some(b_first + i), None, b[i].to_string()),
            (None, Some(j)) => (Mark::Added, None, Some(a_first + j), a[j].to_string()),
            (None, None) => unreachable!("align never yields an empty pair"),
        })
        .collect()
}
