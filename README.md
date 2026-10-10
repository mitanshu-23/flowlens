# flowlens

See how your Rust code flows — without reading the code.

It draws the program as a map: each **feature** (a place where the program
starts doing something), what it calls in the order it calls it, and for every
function the steps it takes, in plain words — calls, decisions, loops and the
ways it can end. The code is always one click away for anyone who does want to
read it.

When the project is in git, the same page also shows **what a change did** —
useful for an edit you did not write line by line, such as an AI's. Not a diff:
each feature's steps before and now, side by side, with the differences
marked, and the features the change did not touch listed as working the same.

See what it looks like in the [screenshots](docs/gallery/README.md).

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
- Files are found by following `mod` declarations. A module declared inside a
  macro — `cfg_if!` blocks, as in `libc` — is not followed, so its code is
  left out.

---

## What it can and cannot do

**It resolves:** free functions, associated functions (`Type::new()`),
`self.method()`, module-qualified paths, calls across files via `use`,
re-export chains (`pub use`), and calls hidden inside macro arguments such as
`format!("{}", helper())`.

**It reads documentation** from `///` and `/** */` comments — the first
sentence is how a function is described in the steps and on hover, the whole
comment shows in its details. Ordinary `//` comments are not documentation
and are ignored.

**It does not do type inference.** A bare `.foo()` on a receiver whose type is
unknown cannot be resolved, and rather than guess, the page leaves it out of
the steps. Cross-file method calls are matched by unique name; about forty
very common `std` names (`len`, `new`, `get`, `iter`, …) are never matched
that way, so `vec.len()` will not bind to your own `len`.

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
| `src/review/page.rs` | The page itself: data, layout and script |
| `src/json.rs` | Just enough JSON writing for the page's embedded data |

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this crate by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
