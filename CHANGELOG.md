# Changelog

All notable changes to flowlens are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/mitanshu-23/flowlens/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/mitanshu-23/flowlens/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mitanshu-23/flowlens/releases/tag/v0.1.0
