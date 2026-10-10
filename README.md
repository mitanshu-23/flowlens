# flowlens

See what your code does now, next to what it did before — without reading the
code.

Built for reviewing changes you did not write line by line, such as an AI
edit. It does not show you a diff. It shows each **feature** of the program,
the steps it took before the change and the steps it takes now, side by side,
with the differences marked. Features that did not change are listed as
working the same, so you can see the edit did not touch them either.

The code is always one click away for anyone who does want to read it.

---

## Quick start

Full step-by-step instructions, including setup and troubleshooting, are in
[RUNNING.md](RUNNING.md).

```bash
cargo install flowlens
cd your-rust-project
flowlens . --review
```

Open `review.html` in a browser. It compares your working copy with the last
commit, so it shows your uncommitted changes. To compare with something else:

```bash
flowlens . --review --against main        # everything since main
flowlens . --review out.html --against v1.2
```

Your repository is never touched: the old version is exported to a temporary
folder and deleted afterwards. With no git history at all — a folder not in
git, or a repository with no commits — you get the project map on its own,
nothing marked as changed.

Pointed at a Cargo workspace root, it puts every member crate on one map.

---

## What the review shows

The page is a **map of the project**: boxes are functions — each labelled
with its name, its description on hover — arrows are calls,
flowing from the features on the left to what they use on the right. Colour
says what changed:

| Box | Meaning |
| --- | --- |
| Amber | Changed |
| Green | New |
| Red, dashed | Removed |
| Dotted amber outline | Unchanged itself, but leads to a change |
| White | Unchanged |

Arrows are coloured the same way: a green arrow is a call that is new, a red
dashed one a call that was removed.

Busy boxes stay readable. A function's calls hang off **one trunk line**,
each on a short straight branch carrying its call number, instead of a fan
of curves. Every call gets its own box, so a box's call numbers run ①②③
without gaps. A box with more than six
calls shows them **a page at a time**, in call order, with a pager after
them — *‹ calls 7–12 of 14 ›* — whose dots mark the pages; an amber dot is a
page holding a change. The box opens on the first such page, turns by itself
to the page of a call you select, and the arrow numbers stay the real call
numbers across pages.

Each function is drawn **once**, where it is first called. Any other place
that calls it shows a small **↪ reference chip** beside the caller instead of
an arrow across the map — so a helper used in ten places does not pull ten
lines through the picture. A call back up the chain (recursion) shows as
**↺ calls back to …**. Hover a chip to see its box light up; click it to go
there.

The header names the project, says how many functions changed (*3 changes ·
1 feature*, or *Current code* in Project flow) and what it is compared with
(*vs HEAD · e479368*; hover for the commit message and date). The map is the
**Main space**;
flows taken out of it go to the side space beside it.

**Two views**, switched at the top:

- **Changes** (the default): only the changed functions and the paths from
  the features down to them. Twenty changes stay twenty boxes, not a
  thousand-line diff.
- **Project flow**: every feature and what it calls directly, plus every
  the shape of the whole program at a glance, as the code is **now**. Nothing
  is compared here: no colours, no *changed* marks, no before and after —
  removed functions are gone and the code shows only its current lines.
  Switch back to *Changes* to compare.

**⇢ / ⇣** (horizontal / vertical) sets the direction: features on the left with calls
flowing right, or features at the top with calls flowing down. Your choice is
remembered in your browser.

**In order:** a function's calls are laid out in the order it makes them —
top to bottom, or left to right when vertical — and the arrows are numbered
①②③, so the map reads like the code runs.

**Selecting:** clicking a box selects it where it is. The map does not
rearrange: the box, what it calls, and the path that leads to it from a
feature stay bright and everything else fades. The view pans only if the box
was off screen.

**Collapsing:** on a map of more than 40 boxes — too dense to read whole —
selecting a box collapses the map to just that box, the path to it, and what
it calls; clicking a neighbour then moves along the flow. On any map, *Focus
on this* in the details collapses it on request, and *Show all* brings the
whole map back with your +N unfolds as you left them. Changes to the layout
glide rather than jump, unless your system asks for reduced motion.

**Side space:** when a flow is too big to read in place, take it out. The
**⇱** button that appears when you hover a box — or *Take out to side space*
in its details — moves the flow starting at that box into its own map beside
the main one. The main map keeps only that box, marked *⇱ in side space*;
what it reaches solely through it goes with it. The side space has its own
pan, zoom, Fit and +N unfolds, and holds several flows as tabs. Selecting a
function anywhere in a taken-out flow shows it there, with its path; ✕ on a
tab puts the flow back.

The **details buttons** — four small icons, the panel drawn on the right, left,
bottom, or as a separate window — put the details panel beside the
map, under it, or in a separate browser window — handy on a second screen:
click around the map in one, read the steps in the other. Drag the divider to
resize. Both choices are remembered in your browser.

Short on room? Click the details icon that is already on (the right-panel one when it is on
the right) and the panel closes: the map takes the whole screen, and
clicking a box opens its details in a box floating over the map, the same
details as the panel, with ✕ (or **Esc**) to close it. The map pans so the
box you picked is not hidden under it. Click any side to bring the panel
back; this choice is remembered too.

**+N** on a box unfolds the functions it calls, so the map grows only where
you look. Drag to pan, scroll to zoom, press `/` to find a function.

The side panel, for the box you clicked, shows its steps in three tabs:
**All steps** (the default), **Main path** and **Code**. The **main path**:
the path the code normally takes runs down one line — calls, values set,
loops — and every branch that only stops hangs off it as a side exit:

```
→ Get setting ("import.batch_size")
◆ If fetch_orders(…) succeeds
   └ ! If it fails → stop with error “Could not fetch orders: …”
≔ Start with all_valid = true
↻ L1 For each order
   │ └ ■ If order.get("id") is missing → set all_valid to false, leave loop L1
   ◇ Depending on kind
   │  ├ If kind is "refund"     … its own lane
   │  └ If kind is "exchange"   … its own lane
◇ Only if all_valid
   │ → Get setting ("import.archive_dir")
```

Loops, *Only if* and *Depending on* lanes fold from their header line (▾/▸);
folding *Depending on* folds every case at once. Folded runs in *All steps* —
*! 3 error stops · show*, *⋯ steps 4–6 did not change · show* — open in
place and fold away again with *· hide*.

A **◆ gate** is a decision where one side carries on and the others only stop.
**Depending on** splits the line into lanes when several sides do real work.
**Only if** marks steps that run only sometimes. A longer exit branch shows
how it ends and opens with *+N steps*. **All steps** shows every step
in code order, numbered like an outline, and **Code** the function's
source — syntax-coloured, with what changed marked + and −, before and now
line numbers side by side. Clicking any step's number opens the code at that
step's line. *‹/› Code* at the top of the details does the same for the
whole function; the main path keeps those numbers as
small hints so references between the two match.

In either view, what changed is marked in place:

```
→  Index the whole crate
◇  If path.is_dir()
!  Stop with an error  Err("file is not part of a crate")     CHANGED
     was: Stop with an error if read_to_string(path) fails
→  Read Cargo.toml                                            REMOVED
→  Index a single file                                        REMOVED
```

Conditions say what they check, in words where the code follows a pattern a
person would describe the same way every time: *If reading the document
**fails***, *If `order.get("id")` **is missing***, *If **Read the
file** (step 2) succeeds* (the call just before, by name and step number). Stops say what happens: *Stop with error
“price is missing for order …”*, *Stop and pass the error on*,
*Finish early*. A branch with a single stop in it is one line — *If Read the
file (step 2) fails → stop and pass the error on*. Anything without such a pattern is
quoted as code, trimmed of arguments and `Option` plumbing.
Anything cut short with *…* — code, arguments, a message — is underlined
with dots: click it to see it whole, and again to shorten it.

Words that point somewhere say where. Loops are named **L1**, **L2** at the
top and **L1.1**, **L1.2** for loops inside L1 — counted among loops alone,
however deep the steps between — and the name sits on the loop's own line.
*Leave the loop* says which: *leave loop L1*, *skip to the next
`order` in L1*, *go round L1.1 again*; hover a name to see the loop.
Rust's labels are followed, so `break 'outer` names the outer loop. *Otherwise* says what it is the alternative to — *(when step 2.14.4
does not hold)*, and a check on the call just before names that call and its
step. An `if` with
nothing to show when it holds but an `else` that does reads the other way
round: *If `by_store.get_mut(sku)` is missing → stop with error …*.

On long functions, runs of error handling fold into one line each
(*! 3 error stops · show*), so the main path reads first; *Show error
handling* opens them all. Nothing that changed is ever folded away.

The overview's Summary ends with **What the signs mean**, which lists every
sign and badge with a line of explanation; it starts open and stays as you
leave it. Hovering any sign, step number, badge or folded line explains it
too. On the main path each step's number sits on the left, beside its sign.

Steps are numbered like an outline — 1, 2, 3, and 3.1, 3.2 inside a branch —
and a branch or loop folds with a click. Hovering a call highlights its box on
the map. Above the steps, **How it can end** counts the ways the function can
finish early — errors, crashes, early returns — and for a changed function
shows what each count was before (*errors: 2 → 3*).

Each function's details show **its own glossary** just before its steps: its **variables** —
name, `mut` if it can change, type where written, what it **starts as** (*chosen
by matching `order.get("kind")`* for a `match`), and every place it
**changes**, the same change said once (*→ `false` at lines 36, 45, 54*), each
line opening the code there — and cards for the **types and values it uses**,
with their doc line, fields or value.

Above the steps, the details show the function's **whole doc comment** (its
first sentence is the label), what it **takes** — each parameter with its
type — and what it **gives back**, and the **types and values it uses**.
Every type name there is a link into the glossary.

The details panel keeps a history, like a browser: **← Back to …** returns to
the previous function, glossary entry or the overview — naming where it goes —
and **Overview** goes straight home. **Alt+←** does Back from the keyboard.
Switching a tab or a step view is not a new place, so Back always moves.

With nothing selected, the details show the overview in two tabs:
**Summary** (what changed and the features it touches, then what the signs
mean) and **Glossary**.

The bar between the main map and the side space drags to share the room
between them — double-click it to split evenly — and is remembered.

**▤ Structure** in the toolbar opens the project as a tree in the side space,
as a tab beside any taken-out flows — folders, files, and everything each file
defines in file order: functions, methods grouped under their type, structs,
enums, traits, constants and statics. Changed items carry their dot, folders
and files count the changes inside them and open on their own when they hold
one, and a filter finds a name or a file. Clicking a function selects it on
the map and opens its details; clicking a type or constant opens its glossary
entry; the selected function is highlighted in the tree, its folders opened.

Lists longer than ten — the glossary, a function's variables and the types
it uses — show ten at a time with a pager, *‹ 1–10 of 16 ›*, whose page
buttons turn amber where a page holds a change.

The **Glossary** lists every type, constant
and static the project defines — `lazy_static!` ones included — with a filter.
An entry shows its doc comment, its fields (each with its own doc), an enum's
choices or a trait's methods, a constant's type and value, which functions
use it, and its definition. An entry the change touched is marked: a struct
that gained a field shows as changed, with its definition before and now.

It also lists what uses the function and what it uses, with new and dropped
calls marked. *Show code* gives the before/after source, for developers.

With nothing selected, the panel lists the affected features and every change,
one line each; clicking one finds it on the map.

**A feature** is an entry point: a function nothing else in the program calls,
such as `main`, a request handler, or a public API function. Tests and trait
methods like `fmt` or `drop` are not features. A function that loses its last
caller becomes one — often the sign of code an edit left behind.

**A step** is something you would mention when describing what a function
does:

| Step | Shown as |
| --- | --- |
| A call to one of your own functions | → its doc comment's first sentence, or its name spelled out |
| A branch or loop | ◇ *If …*, *Otherwise*, *When it is …*; ↻ *For each …* |
| `return`, `return Err(..)`, a trailing `Err(..)` | ■ *Stop here*, ! *Stop with an error* |
| `?` | *stops if it fails*, on the call it follows |
| `panic!`, `.unwrap()`, `.expect()`, `assert!` | ✕ *Crash if …* |
| `todo!()`, `unimplemented!()` | ✕ *Not built yet* |
| Setting a variable a later decision reads, and that changes along the way (`let mut all_valid = true`, `all_valid = false`, `retry_count -= 1`) | ≔ *Start with …*, *Set … to …*, *Lower … by …* |

Calls into `std` and dependencies are not steps; they are how a step is done.
Neither are ordinary assignments — except for flags and counters: a variable
that a decision reads *and* that the function changes along the way. Those
are followed, so a decision like *If `all_valid`* says where the value
came from: *· `all_valid` set in steps 4.1, 4.2.1, 4.2.2 +5*.

**Changes the steps cannot show** — a different constant, an edited condition
on a branch that calls nothing of yours — are still caught: the box turns
amber and the panel says to look at the code.

**Tests edited** alongside the code are listed separately: a changed test can
hide a feature that broke.

### Limits

- Rust only.
- Resolution is by name, with no type inference (see
  [What it can and cannot do](#what-it-can-and-cannot-do)). A method call the
  analyzer cannot pin to one definition is not a step, so a change behind it
  shows as *details changed* rather than as a step.
- It shows what the code *says* it does. It does not run anything yet, so it
  cannot show different results for the same input.

---

# Exploring a codebase

The rest of the tool answers a different question: how does this crate fit
together? It indexes the code the way an IDE does and shows how **your own**
functions connect.

```bash
flowlens . --html map.html
```

## What you get

One self-contained page — no server, no network, no build step. For every
function:

- **Signature**, one parameter per line with its type, and the return type
- **Documentation**, the full `///` comment
- **Called by** — every caller, with file and line
- **Calls in order** — the body read top to bottom (see below)
- **A small map** — callers above, callees below, all clickable

### Calls in order

Not an alphabetical list — the calls in the order they run, each showing the
arguments passed, what comes back, and the variable it lands in:

```
 94   lines = LineIndex::new(src) → Self
105   (imports, globs) = collect_imports(&root) → (HashMap<String, String>, Vec<String>)
106   symbols = collect_symbols(&root, src, &lines) → Vec<Symbol>
107   lookup = Lookup::build(&symbols) → Self
108   references = collect_references(&root, src, &lines, …) → Vec<Reference>
```

Calls are **nested inside the loops and branches they actually sit in**, so the
list reads as the shape of the body rather than a flat sequence:

```
 42   setup()
  ┌ for f in &ws.files                              runs repeatedly
  │ 45   index = index_file_in(&display, &src, …) → FileIndex
  │  ┌ if f.is_root()                               may not run
  │  │ 48   register(index)
  │  └
  └
 51   finish()
```

A call inside a `for` runs many times; a call inside an `if` may not run at
all. `if` and `else` are told apart, `match` arms carry their pattern, and a
closure body is marked deferred rather than conditional. Context stops at the
function boundary — a nested `fn` starts fresh.

The value is followed through `?`, `.await`, casts and postfix calls, so
`let ws = Workspace::load(p)?` credits `ws` to `Workspace::load`. A call used
as an argument, a condition, or a match scrutinee stores nothing and says so
rather than borrowing an unrelated name.

`std` and dependency calls are left out by default. The checkbox above the list
folds them back in, in the same order, so you can read the real sequence when
you need to.

### Hiding what you do not care about

Hovering a function in the sidebar shows a `×`; hovering a filename shows
**hide file**. Either removes it from the sidebar, from every flow list, and
from every caller list — useful for pushing generated code, test modules, or a
noisy helper out of the way.

A bar at the top of the sidebar counts what is hidden and restores it with
**show all**. The choice is kept per crate in `localStorage`, so it survives a
reload.

Search filters by name, file, or doc text (press `/`). Checkboxes narrow to
public or documented functions. Clicking any name jumps to it, so you can walk
the call chain. The URL updates, so a link lands where you left it.

If nothing is selected, the page opens on **entry points** — functions nothing
else calls, which are the natural places to start reading an unfamiliar crate.

### Why not just a big graph?

There is one (`--dot`), but for this crate it renders 138 nodes at
3968×5060 points. Technically complete, practically a wall. The interactive
page shows one function at a time, which is the question you actually have.

---

## The text report

If you would rather read than click:

```bash
flowlens .
```

Prints to the terminal. The `.` is the path to analyze. Given a directory, the `mod` tree is
followed from the crate root and calls are resolved across files.

---

## Saving the output

The report goes to stdout, so redirect it:

```bash
mkdir -p out
flowlens . > out/report.txt
```

The report runs to about 1,100 lines for this crate. To shorten it:

```bash
flowlens . --stats          # about 15 lines
flowlens . --no-outline     # drops the definition listing
flowlens . | less        # page through it
```

---

## Seeing the graph

Two steps: generate a `.dot` file, then render it. A `.dot` file is a **text
description** of a graph, not an image.

### 1. Generate

```bash
mkdir -p out
flowlens . --files out/deps.dot     # file dependency graph
flowlens . --dot   out/calls.dot    # function call graph
```

`mkdir -p out` first — the tool errors rather than creating the directory.

**Start with `deps.dot`.** It has one node per file (10 for this crate) and is
readable at a glance. `calls.dot` has a node per function — 127 here — and is
a wall unless you are hunting something specific.

### 2. Render

**Option A — online, nothing to install.**

Open <https://dreampuf.github.io/GraphvizOnline>, clear the sample, and paste
the contents of your `.dot` file.

**Option B — locally.**

```bash
sudo apt install graphviz          # not installed by default

dot -Tsvg out/deps.dot  -o out/deps.svg
dot -Tsvg out/calls.dot -o out/calls.svg
```

Then open the `.svg`:

```bash
xdg-open out/deps.svg      # or: firefox out/deps.svg
```

SVG rather than PNG so you can zoom without it going blurry — which matters
once a graph has a hundred nodes.

If `dot` gives a cramped layout on something large, try the other engines:
`fdp`, `sfdp`, `neato`, `twopi`. Same flags.

### What a node tells you

```
        index_file
        L81
        ———
        Index a single file with
        no crate context.
```

Name, where it is defined, then the first sentence of its doc comment. Fill
colour is visibility — green `pub`, amber `pub(crate)`, grey private, blue for
`main`.

**Hover a node in the SVG** for the full picture: the complete signature and
the entire doc comment, not just the summary. This only works in a browser, so
it is a reason to render `-Tsvg` rather than `-Tpng`.

Undocumented functions simply show name and line. If the graph looks bare,
that is the codebase telling you something — the report header prints a
documentation coverage figure for exactly this reason:

```
29/43 public function(s) documented (67%)
```

Doc summaries widen the nodes. For a tighter layout on a large crate:

```bash
flowlens . --dot out/calls.dot --no-docs
```

Tooltips keep the full documentation either way.

---

## Reading the report

Sections, in order:

| Section | What it tells you |
| --- | --- |
| `FILES` | One row per file: definitions, references out, references in |
| `FILE INTERLINKING` | Which files depend on which, and how heavily |
| `OUTLINE` | Every definition, grouped by module and `impl` block, with its doc summary |
| `USER-DEFINED CALLS` | **The main event** — what each function calls, your code only |
| `CALLERS OF EACH DEFINITION` | The reverse: who calls this? (crate-wide) |
| `ENTRY POINTS & UNREFERENCED` | Roots, and dead-code candidates |
| `LIBRARY DEPENDENCIES` | `std` and crates, summarised and kept separate |
| `DIAGNOSTICS` | What could not be resolved, and why |

Markers in the call sections:

```
→   calls something in the same file
⇒   calls something in another file of this crate
⇢   matched by unique method name — a good bet, but a bet
?   the name matches several definitions
```

---

## Options

```
--review [OUT.html] before/after review of each feature (default review.html)
--against <REF>     what "before" means for --review (default HEAD)
--html <OUT.html>   interactive explorer — the readable option
--dot <OUT.dot>     write the call graph instead of the report
--files <OUT.dot>   write the file-dependency graph (best for large crates)
--all               include std and dependency calls, excluded by default
--no-outline        skip the per-file definition listing
--no-docs           leave doc summaries off the graph, for a compact layout
--cfg <OUT.dot>     write per-function control-flow graphs
--stats             print counts only
-h, --help          usage
```

### Control-flow graphs

A different view: the branches and loops *inside* each function, rather than
the calls between them.

```bash
flowlens src/cfg.rs --cfg out/flow.dot
dot -Tsvg out/flow.dot -o out/flow.svg
```

`--cfg` only ever reads the first file, so it is always single-file.

---

## Single-file analysis

Passing a file that belongs to a crate still indexes the **whole crate**, so
that cross-file calls resolve. That is usually what you want.

For genuinely isolated analysis, use a file outside any crate:

```bash
cp src/index.rs /tmp/one.rs
flowlens /tmp/one.rs
```

Expect a sparse result. A single file cannot see the functions it calls in
other files, so those calls are reported as unresolved — which is the honest
answer from that file alone.

---

## What it can and cannot do

**It resolves:** free functions, associated functions (`Type::new()`),
`self.method()`, module-qualified paths, calls across files via `use`,
re-export chains (`pub use`), and calls hidden inside macro arguments such as
`format!("{}", helper())`.

**It reads documentation** from `///` and `/** */` comments — the first
sentence onto the graph node, the whole thing into the hover tooltip and the
outline. Ordinary `//` comments are not documentation and are ignored.

**It does not do type inference.** A bare `.foo()` on a receiver whose type is
unknown cannot be resolved. Rather than guess, the analyzer reports it — as
`?` when several definitions share the name, or in the `DIAGNOSTICS` count
when there is nothing to match. Cross-file method calls are matched by unique
name and marked `⇢`; about forty very common `std` names (`len`, `new`, `get`,
`iter`, …) are never matched that way, so `vec.len()` will not bind to your
own `len`.

Read the `DIAGNOSTICS` section before trusting a dead-code claim.

---

## Development

From a copy of the source:

```bash
cargo test
cargo build --release
```

The tests pin resolution rules that are easy to break silently — run them
after touching `src/index.rs` or `src/workspace.rs`. `tests/review.rs` pins
step extraction and the before/after comparison; its git tests need `git` and
`tar` on the path.

### Layout

| File | Role |
| --- | --- |
| `src/model.rs` | Core types: `Symbol`, `Reference`, `Resolution` |
| `src/provenance.rs` | Tells your code from `std` and dependencies |
| `src/index.rs` | Indexes one file: definitions, references, local resolution |
| `src/modmap.rs` | Finds a crate's files by following `mod` declarations |
| `src/workspace.rs` | Links files together; cross-file resolution |
| `src/review/mod.rs` | Before/after review: features, statuses, what changed inside |
| `src/review/flow.rs` | The steps a function takes: calls, branches, exits |
| `src/review/compare.rs` | Lines up two versions' steps (and source lines) |
| `src/review/git.rs` | Exports the "before" version with `git archive` |
| `src/review/glossary.rs` | Types, constants and statics: docs, fields, who uses them |
| `src/review/page.rs` | The review page |
| `src/json.rs` | JSON helpers shared by both pages |
| `src/report.rs` | The text report |
| `src/html.rs` | The interactive explorer |
| `src/dot.rs` | Graphviz output |
| `src/cfg.rs` | Per-function control-flow graphs |

Note: `out/` and `*.dot` are in `.gitignore`, so generated files may be hidden
in your editor's sidebar. Check with `ls out/`.

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this crate by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
