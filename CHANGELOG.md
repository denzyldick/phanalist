# Changelog

## [Unreleased]

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
