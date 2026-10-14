# Running flowlens

Step-by-step instructions for installing the tool and using it on a Rust
project. For what the output means, see [README.md](README.md).

---

## 1. What you need

| Tool | Why | Check with |
| --- | --- | --- |
| Rust (stable, 1.86 or newer) | installs the tool | `cargo --version` |
| git | fetches the "before" version for a review | `git --version` |
| A web browser | opens the generated page | — |

No Rust yet? Install it from <https://rustup.rs>:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## 2. Install it

```bash
cargo install flowlens
```

This puts `flowlens` in `~/.cargo/bin`, which rustup adds to your `PATH`.
Run the same command again later to update it.

From a copy of the source instead:

```bash
cargo install --path .
```

---

## 3. Review a change

The review compares your code with an earlier commit. If the project is not in
git yet, or has no commits, it still runs: you get a map of the project as it
is, with nothing marked as changed (see [Just the project map](#just-the-project-map)).

```bash
cd ~/path/to/your-rust-project

# 1. Make sure there is a commit to compare against
git add -A && git commit -m "before the change"     # only if you have none yet

# 2. Make the change — by hand or with an AI — and leave it uncommitted

# 3. Generate the review
flowlens . --review

# 4. Open it
xdg-open review.html        # Linux
open review.html            # macOS
start review.html           # Windows
```

### Comparing with something other than the last commit

```bash
flowlens . --review --against HEAD~3       # the last three commits plus uncommitted work
flowlens . --review --against main         # everything on this branch since main
flowlens . --review --against v1.2         # since a tag
```

### Choosing the output file

```bash
flowlens . --review out/upload-change.html
```

The name must end in `.html`. The folder must already exist.

### Cargo workspaces

Point it at the workspace root (the folder whose `Cargo.toml` has
`[workspace]`) and every member crate goes on one map, each function labelled
with its crate (`api · main`, `worker · main`). To look at just one crate,
point it at that crate's folder:

```bash
flowlens . --review                  # every member
flowlens crates/worker --review      # just this one
```

### Just the project map

In a folder that is not in git, or a repository with no commits, the same
command draws the project as it is: each feature on the left, what it calls to
the right, every box clickable. Nothing is marked as changed, because there is
nothing to compare with.

```bash
flowlens . --review
```

---

## 4. Using the page

- **Changes / Project flow** (top right) switch between only what changed and
  the overall shape of the program. *Changes* is greyed out when there is no
  git history to compare with.
- **⇢ / ⇣** (horizontal / vertical) sets which way the map flows. The choice is
  remembered in your browser.
- **Click a box** to select it: it stays where it is, it unfolds the
  functions it calls, its path and calls stay bright, everything else fades,
  and the details show its code and steps. On a big map
  (over 40 boxes) the map also collapses to just that box and its neighbours.
  *Focus on this* collapses it on any map; *Show all* or **Esc** undoes it.
- **The details icons** (panel on the right, left, bottom, or in its own
  window — hover for the name) move the details panel. *Window*
  opens it in its own browser window (allow pop-ups if your browser asks);
  close that window to dock it back. Drag the divider between map and details
  to resize. Clicking the details icon that is already on (say the right-panel one) closes
  the panel so the map gets the whole screen; clicking a box then opens its
  details in a box over the map (✕ or **Esc** closes it). Click any side to
  bring the panel back.
- **+N** on a box shows the functions it calls; **−** folds them back, with
  everything opened below them.
- **⇱** (hover a box) takes the flow starting at that box out into a side
  space next to the map, so a big flow can be read on its own. ✕ on its tab
  puts it back.
- **Drag** to move, **scroll** to zoom, **Fit** to see everything.
- **`/`** jumps to the search box; **Esc** clears the selection.
- **Theme** and **Font** (top right) change the look — the Rust book's
  themes, Grotesk or Monospace — and are remembered in your browser.
  **Signs** explains every sign and colour on the page.

The page is a single file with nothing to install and no internet needed. You
can send it to someone else as is.

---

## 5. Running the tests

```bash
cargo test
```

All tests should pass. The review tests create small git repositories in your
system's temp folder, so they need `git` too.

---

## Troubleshooting

| Message | What to do |
| --- | --- |
| `project map → ... (this folder is not in git yet ...)` | Not an error: you get the project map. To see changes, `git init`, commit once, then make your change. |
| `project map → ... (this repository has no commits yet ...)` | Same: commit once (`git add -A && git commit -m init`), then make your change. |
| `cannot find 'X' in this repository` | The `--against` name is wrong; check it with `git log --oneline` or `git branch`. |
| `found no Rust files under ...` | Point it at the crate or workspace folder (the one with `Cargo.toml`), not its parent. |
| `cannot write review.html` | The output folder does not exist; create it with `mkdir -p`. |
| `nothing to do: add --review ...` | Add `--review`: `flowlens . --review`. |
| `the review page's name must end in .html` | Name the page `something.html`, or leave the name out for `review.html`. |
| The page says *nothing changed*, but you changed something | The change is not in Rust code the crate builds (for example a test file under `tests/`, or a file not reachable through `mod`), or it was already committed. Try `--against HEAD~1`. |
| `error[E0...]` while building | Update Rust: `rustup update stable`. |
