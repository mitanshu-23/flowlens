# Changelog

All notable changes to flowlens are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.2] - 2026-10-14

### Added

- **Route handlers and other functions passed by name** — `get(handler)`,
  callbacks, tasks given to `spawn` — are linked: a step *Hand handler to
  `get`* with an arrow on the map. A change inside a handler now marks its
  feature as changed.
- **Clicking a box unfolds what it calls**, as its +N would.
- **Links in the code view**: a call to one of your functions opens it; a
  variable or glossary value shows where it starts.
- **Themes** from the Rust book — Light, Rust, Coal, Navy and Ayu — and a
  **Font** menu (Grotesk, Monospace), remembered in the browser.
- A logo, in the header and as the browser-tab icon.

### Changed

- The header has two rows: the project and the page settings (Theme, Font,
  Signs) on top, the map tools below. *What the signs mean* moved from the
  Summary into the header.
- A function's details open on its **Code**, then *All steps* and *Main
  path*. A wide panel uses two columns: takes, gives back and how it can end
  across the top; the code beside its variables; used by and uses at the end.
- The code is shown whole — long lines wrap and indentation scales to fit —
  with no scrolling box of its own; one line-number column unless the
  function changed.
- A function with no steps of its own shows its code instead of an empty
  space.

### Fixed

- **−** on a box now folds everything opened below it.
- Names on the map no longer run past their box in wider fonts; for
  `Type::method` the method is kept.
- No square outline around a clicked (rounded) box.

## [0.1.1] - 2026-10-12

### Added

- **Search marks every place** a function appears on the map, not just the
  first; Enter / Shift+Enter step through them, with a count of matches.
- **Regular-expression search**: a toggle or `/pattern/`, plus a collapsible
  pattern helper with ready-made patterns and your own saved ones.
- **Where each function lives**: boxes show the file name. When two files
  have the same name, the first differing parent folder is added. Hovering
  shows the full path, line and module.
- **Hide and restore** functions on the map, along with what is reached only
  through them. This is kept per project, with an undo. Call numbers skip
  hidden calls.
- **Clickable names in the steps**: variables and glossary entries mentioned
  in a step jump to where they are declared.
- **What to review first**: changes and features are ranked by impact (reach
  and risk), with the reasons in words. *By file* keeps file order.
- Features with the same name are told apart by type or location.

### Fixed

- The Changes view could crash on load in large projects (a pager left
  without its box when a branch was folded).
- Files excluded with `export-ignore` in `.gitattributes` no longer show up as
  new. The "before" version is now read straight from git's object store, so
  `tar` is no longer needed.

### Changed

- The README is shorter; the page explains its own signs.

## [0.1.0] - 2026-10-10

First release.

[Unreleased]: https://github.com/mitanshu-23/flowlens/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/mitanshu-23/flowlens/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/mitanshu-23/flowlens/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mitanshu-23/flowlens/releases/tag/v0.1.0
