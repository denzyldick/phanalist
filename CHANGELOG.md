# Changelog

## [Unreleased]

### Added

- The Z3-backed formal verification engine is split into one rule per finding class: `E0031` division by zero, `E0032` modulo by zero, `E0033` `intdiv()` by zero, `E0034` `intdiv()` overflow, `E0035` missing array key, `E0036` array index out of bounds, `E0037` string offset out of bounds, `E0038` method call on null, `E0039` property access on null, `E0040` return contract violation, `E0041` unreachable code, and `E0042` impossible assertion. Each code has its own settings block and can be enabled or disabled independently. Enabling or disabling only `E0031` expands to all verification codes for backward compatibility.
- The shared formal verification engine proves division, modulo and `intdiv()` safety; array and string bounds; null method and property access; declared and PHPDoc return contracts; PHPDoc parameter and `@var` refinements; `@phpstan-assert`/`@psalm-assert` contracts; assertions; unreachable statements; `switch`/`match` guards; numbered `break`/`continue`; per-element `foreach` analysis; closures and arrow functions; `try`/`catch`/`finally`; `never` return contracts; and `strlen()`/`count()` derived bounds. It unrolls `while`, `for`, `do`/`while`, and literal-array `foreach` loops, and uses project-wide constant-return and identity function and method summaries, including inherited and trait methods, `parent::` calls, and class and function `use` aliases, and reports unverified properties according to `unknown_policy`.
- Vendored Z3 so the verifier builds without a system Z3 installation. Release binaries now target GNU Linux (musl builds are no longer produced).
- The verification family propagates bounded integer ranges through direct same-class helper calls, including expressions such as `$max - $min + 1` and `max()`, `min()`, `abs()`, and `intdiv()`.
- The verification family propagates PHPDoc return contracts to callers, including numeric ranges, non-empty strings/lists, and shaped arrays; documented non-empty values carry length information for `strlen()` and `count()`.
- The verification family proves bounds for PHPDoc `list<T>` and `non-empty-list<T>` values, including `count()`/`sizeof()` lengths, appends, `foreach` keys, `isset()` guards, and guards in short-circuited `&&` chains. Reachable symbolic out-of-bounds indexes report `array-index`; indexes that cannot be modeled report `unverified` under `unknown_policy: report`.
- The verification family merges loop exit values with the feasible zero-iteration path for integer counters initialized to zero, so a counter initialized before a possibly empty loop is still considered possibly zero; unknown accumulators are reported as `unverified` under `unknown_policy: report` instead of being treated as safe. Unknown numeric accumulators retain enough type information to prove guards such as `$count > 0`, while strict type-sensitive guards remain unverified. The benchmark now includes the PPOE/pgacc-style loop regression, guarded counterparts, and a strict-guard soundness case.
- The verification family propagates constant-return and PHPDoc method contracts through directly instantiated objects stored in variables and through parameters whose class has no known project-local subclass, including classes and contracts declared in other indexed files.
- The verification family now reports solver `unknown` results with clearer `unverified` guidance by default; `unknown_policy: ignore` still suppresses them.
- The verification family derives exact list lengths through `array_merge()` for list inputs, preserves lengths through `array_map()`, `array_values()`, and `array_reverse()`, and derives string lengths through `str_repeat()` with a literal repetition count. It also tracks generic `array<K,V>` lengths, bounds `array_diff()`/`array_intersect()`/`array_unique()`/`array_filter()`/`array_fill_keys()` results by their input length, keeps `array_keys()` lengths, models `array_fill()`, `array_pad()`, `array_slice()`, and `array_chunk()` lengths, uses the longest input for multi-array `array_map()`, derives `substr()`, `str_split()`, and `str_pad()` result lengths from PHP's offset, chunking, and padding rules, and resolves `array_key_first()`/`array_key_last()` keys for tracked literal arrays in PHP insertion order. These lengths and keys feed `E0035`, `E0036`, and `E0037` bounds checks.

### Fixed

- Full-project scans (`--src .`) no longer grow memory with the size of the tree. Every scanned file was parsed into a single shared arena that was only freed when the scan ended, so peak memory tracked the combined size of all parsed PHP files and could exhaust the host's RAM and swap. Both the indexing pre-pass and the main pass now reclaim each file's AST before reading the next one, so peak memory is proportional to the largest single file rather than the whole project.
- A single file whose name is not valid UTF-8 no longer aborts the walk. The scanner used to call `unwrap()` on the file name, so one such file panicked the directory-walking thread and the run continued with a silently truncated file set while still reporting success. A scan of `.` walks `vendor`, `node_modules` and build output, where such names are easy to find.
- Directory walk errors are now skipped with a `-v` diagnostic instead of panicking the walk.
- Each violation is stored once instead of being cloned while building the report, and output pruning no longer duplicates the entire results map. Together these roughly halve peak memory on projects that report many violations.
- `check_types` now controls PHPDoc `@param`/`@var` refinements instead of being ignored; native PHP type hints are always honoured.
- The verification family no longer treats an unknown value as failing `is_*()` type predicates, which caused false "assertion can never hold" findings on dynamic values.
- The verification family no longer marks code after `??`, `&&`, `||`, ternary, or `match` branches as unreachable when the branch merely throws on one path.
- The verification family no longer checks `return;` inside generators against the generator return type, and PHPDoc return types containing spaces in generics or shaped arrays are parsed in full.
- Assertion diagnostics in the verification family only report assertions that provably cannot hold; unknown assertion conditions are treated as assumptions.
- The verification family reads elements from tracked literal arrays, so `??` defaults, divisions, and bounds checks use the element values instead of an opaque result.
- The verification family keeps known-true conjuncts from a short-circuited `&&` chain while checking later operands, so a bounds guard after an unknown condition still proves the access in-bounds; state after the whole expression remains conservative.
- The verification family no longer reports a literal list access as a hard out-of-bounds finding when its index cannot be proven out of bounds; symbolic indexes remain potential findings, and unmodeled indexes remain `unverified`.
- The verification family uses constant arguments from direct same-class `self::`, `static::`, or `ClassName::` calls when analysing private static methods, plus protected static methods when the analyzed project has no subclass of their class. It handles finite sets for multiple literal call sites and named arguments; classes using traits, first-class callables, dynamic selectors, by-reference parameters, and variadic parameters disable this inference. This removes false zero-divisor findings for helpers such as Composer's `convertVersionId` and Laravel's cron-field shifting.
- The verification family tracks simple array element writes and appends, and forgets arrays passed to calls that may mutate them by reference. This removes false array-key findings on `preg_match()`, `preg_match_all()`, and similar code.
- The verification family keeps nested array shapes across branches, so nested missing-key checks remain enabled without stale shapes after branch-local writes. Nested array elements passed by reference to calls are forgotten at the root, and a by-reference closure capture is forgotten only when the closure body actually writes it.
- The verification family no longer reports `false` as incompatible with native `false|string` return hints, or `null` as incompatible with unconstrained template return types such as `TValue|TGetDefault`.
- The verification family collects summaries from `switch` cases, so a method whose early branch returns `null` is not summarized as always returning `null` when other cases return a value.
- The verification family treats an empty `switch` `default` as a fallthrough to the next case, or as an exit when it is last, so an exhaustive switch is no longer treated as falling through to the following statement.
- The verification family analyses non-empty `switch` cases in source order, so statements after a case are also checked with the values written by the preceding case; `break`, `continue`, returns, and exits still stop the fallthrough.
- The verification family treats `null === $value` and `null !== $value` as unknown when `$value` is unknown instead of assuming the comparison is false, preventing false unreachable-code findings.
- The verification family narrows `isset()` on tracked literal array keys, including nested literal shapes, preventing false array-key findings in guarded increment and nested-read patterns.
- The verification family caches its project-wide summary snapshot and parent map, so validation is no longer quadratic in the number of indexed files.
- The LSP workspace indexer reclaims each file's AST as it indexes, so opening a large project in an editor no longer loads every file's parse tree at once.
- The LSP server now restores a document's cross-file summaries from disk when it is closed, reindexes watched files on change or creation, and drops summaries when a file is deleted, instead of leaving stale interprocedural data behind.

### Changed

- Baselines created before the E0031 split store aggregate `E0031` findings. Regenerate them so per-class `E0032`-`E0042` findings are tracked under their own codes.
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
