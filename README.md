# flowlens

See how your Rust code flows — without reading the code.

It writes one page: a map of the program's **features**, what each one calls
in order, and for every function the steps it takes, in plain words. The code
is always one click away for anyone who does want to read it.

When the project is in git, the same page shows **what a change did** —
useful for an edit you did not write line by line, such as an AI's. Not a
diff: each feature's steps before and now, the differences marked, and the
features the change did not touch listed as working the same.

See what it looks like in the [screenshots](docs/gallery/README.md).

---

## Quick start

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

- Your repository is never touched: the old version is exported to a
  temporary folder and deleted afterwards.
- No git history — a folder not in git, or a repository with no commits — gives
  the project map on its own, nothing marked as changed.
- Pointed at a Cargo workspace root, it puts every member crate on one map.

The page is a single file: no server, no network, nothing to install to view
it. Everything on it explains itself on hover, and the signs are listed on
the page.

Setup and troubleshooting: [RUNNING.md](RUNNING.md).

---

## What you get

- **A map** of the project, from each feature to what it calls, in call
  order, coloured by what changed. Two views: **Changes** (only what changed
  and the paths to it) and **Project flow** (the whole program as it is now).
- **Each function's steps** in plain words — its calls, decisions, loops and
  the ways it can end — next to its code, which opens first: as every step in
  order, or as the main path it normally takes.
- **Themes** from the Rust book — Light, Rust, Coal, Navy and Ayu; by default
  Light, or Navy when your system is dark.
- **A glossary** of the project's types, constants and statics, and for each
  function its variables: where each starts and where it changes.
- **The project's structure** — folders, files and what each defines — to jump
  straight to any function or type.
- **What to review first**: on a large change, every change and feature is
  ranked by impact — how many features it reaches and how risky it is (removed,
  different inputs, new ways to crash or fail, steps changed) — with the
  reasons in words; *By file* lists them in file order instead.
- **Hide what you don't need**: take a function off the map (and what is
  reached only through it), then restore it any time — kept per project.
- **Search** by name or regular expression, marking every place a function
  appears, with ready-made patterns and your own saved ones.

Two words the page relies on:

- **A feature** is an entry point: a function nothing else in the program
  calls, such as `main`, a request handler, or a public API function. A
  function that loses its last caller becomes one — often the sign of code an
  edit left behind.
- **A step** is something you would mention when describing what a function
  does: a call to one of *your* functions, a decision, a loop, or a way it
  stops — returning, failing, crashing. Calls into `std` and dependencies are
  how a step is done, not steps. A change the steps cannot show — a different
  constant, say — is still caught, and marked as a change inside.

---

## Limits

- Rust only.
- No type inference: resolution is by name. A method call that cannot be pinned
  to one definition is left out of the steps rather than guessed; cross-file
  method calls are matched by unique name, and common `std` names (`len`,
  `new`, `get`, `iter`, …) never are.
- Calls through trait objects or macro-generated code may not show. A
  function passed on by name — a route handler in `get(handler)`, a callback —
  shows as handed over, not as a call made there.
- Files are found by following `mod` declarations; a module declared inside a
  macro (`cfg_if!`, as in `libc`) is not followed.
- It shows what the code *says* it does. It does not run anything, so it
  cannot show different results for the same input.

---

## Development

From a copy of the source:

```bash
cargo test
cargo build --release
```

`tests/analysis.rs` pins name resolution — run it after touching
`src/index.rs` or `src/workspace.rs`. `tests/review.rs` pins the steps and the
before/after comparison; its git tests need `git` on the path, and a
syntax check of the page's script runs when Node is installed.

| File | Role |
| --- | --- |
| `src/model.rs` | Core types: `Symbol`, `Reference`, `Resolution` |
| `src/provenance.rs` | Tells your code from `std` and dependencies |
| `src/index.rs` | Indexes one file: definitions, references, local resolution |
| `src/modmap.rs` | Finds a crate's files by following `mod` declarations |
| `src/workspace.rs` | Links files together; cross-file resolution |
| `src/review/mod.rs` | Features, statuses, what changed inside |
| `src/review/flow.rs` | The steps a function takes |
| `src/review/compare.rs` | Lines up two versions' steps and source lines |
| `src/review/git.rs` | Reads the "before" version straight from git's object store |
| `src/review/glossary.rs` | Types, constants and statics, and who uses them |
| `src/review/page.rs` | The page: data, layout and script |
| `src/json.rs` | JSON writing for the page's embedded data |

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this crate by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
