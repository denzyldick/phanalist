# Changelog

## [Unreleased]

### Fixed

- Full-project scans (`--src .`) no longer grow memory with the size of the tree. Every scanned file was parsed into a single shared arena that was only freed when the scan ended, so peak memory tracked the combined size of all parsed PHP files and could exhaust the host's RAM and swap. Both the indexing pre-pass and the main pass now reclaim each file's AST before reading the next one, so peak memory is proportional to the largest single file rather than the whole project.
- A single file whose name is not valid UTF-8 no longer aborts the walk. The scanner used to call `unwrap()` on the file name, so one such file panicked the directory-walking thread and the run continued with a silently truncated file set while still reporting success. A scan of `.` walks `vendor`, `node_modules` and build output, where such names are easy to find.
- Directory walk errors are now skipped with a `-v` diagnostic instead of panicking the walk.
- Each violation is stored once instead of being cloned while building the report, and output pruning no longer duplicates the entire results map. Together these roughly halve peak memory on projects that report many violations.
- The LSP workspace indexer reclaims each file's AST as it indexes, so opening a large project in an editor no longer loads every file's parse tree at once.

### Changed

- The progress bar counts the PHP files that will be analysed instead of every directory entry passed along the way, and sizing it no longer walks the tree an extra time.
- Files with no violations no longer reserve an entry (and a path string) in the report.
- Rules that resolve references across files (`E0014`, `E0020`–`E0023`, `E0029`) are unaffected: the indexing pre-pass still completes over the whole project before any file is validated.

## [1.0.0] - 2026-06-14

### Added

- LCOM4 metric (E0015)
- Cognitive complexity metric (E0016)
- CBO, WMC, RFC, DIT, NOC, Ca/Ce, I/A/D architectural metrics (E0017–E0023)
- Lines of Code per Method / per File (E0024–E0025)
- Comment ratio rule (E0026)
- God class / brain class detection (E0027)
- Data class detection (E0028)
- Fan-in / fan-out metric (E0029)
- Cyclomatic complexity density (E0030)
- Config merge logic for upgrading existing configs with new rule defaults
- CD scripts: versioning, publication, and changelog management

### Changed

- Updated CI actions to latest versions (checkout@v4, dtolnay/rust-toolchain, docker/login@v3, upload-sarif@v3)
- Added `github-actions` ecosystem to Dependabot
- Standardized all rule `CODE` visibility to `pub(crate) static`
- Switched macOS x86_64 runner from `macos-13` to `macos-latest`
- Updated README rules table from 24 to 31 rules with correct links
- Fixed broken rule doc links (E0004, E0005) and standardized all paths with leading `/`

### Fixed

- E0016 description from "Using unserialize" to "Cognitive complexity"
- 11 Clippy warnings across e26.rs, e27.rs, e28.rs
- SARIF help URIs to use correct `eN/eN.md` path format
- Typos: `travers_statements_to_validate` → `traverse_statements_to_validate`, `explenation` → `explanation`, `writting` → `written`

### Removed

- Dead file `src/rules/ast_child_statements.rs` and related commented-out imports
- Unused `walkdir` dependency

## [0.1.24] - yyyy-mm-dd

### Added

- Initial release of Phanalist
