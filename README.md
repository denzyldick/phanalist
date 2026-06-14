<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/docs/branding/banner-cropped.png"/>

[![Crates.io](https://img.shields.io/crates/v/phanalist.svg)](https://crates.io/crates/phanalist)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![CI](https://github.com/denzyldick/phanalist/actions/workflows/rust.yml/badge.svg)](https://github.com/denzyldick/phanalist/actions)

> Performant static analyzer for PHP, written in Rust. Catches common mistakes and enforces best practices with zero configuration required.

---

### 🤔 Why Phanalist?

PHP codebases grow. As they grow, they accumulate technical debt — god classes that do everything, methods no one can follow, hidden complexity that breaks with every change. Traditional linters catch syntax errors and style issues, but they don't tell you if your code is **maintainable**.

Phanalist focuses on **structural health**. It measures what matters for long-term maintainability:

- **Complexity metrics** — cyclomatic complexity, cognitive complexity, LOC per method, nested paths
- **Coupling & cohesion** — Law of Demeter violations, god classes, data classes, fan-in/fan-out
- **Object-oriented design** — depth of inheritance, weighted methods per class, response for a class
- **Readability** — comment ratios, error suppression, method parameter counts

Think of it as a health checkup for your PHP code. It doesn't just tell you *that* something is wrong — each rule explains *why* it matters and *how* to fix it.

---

### ✨ Features

- 🚀 **Fast** — built in Rust, analyzes large codebases in seconds
- 🔍 **31 built-in rules** — covering complexity, style, design patterns, and more
- ⚙️ **Zero config to start** — works out of the box, configure only what you need
- 📄 **Multiple output formats** — `text`, `json`, `sarif` (for CI pipelines), and `codeclimate` (for Code Quality platforms)
- 🔌 **Extensible** — adding a custom rule takes minutes

---

### Installation

The simplest way to install Phanalist is to use the installation script:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/denzyldick/phanalist/main/bin/init.sh | sh
```

It will automatically download the executable for your platform:

```bash
$ ~/phanalist -V
phanalist 0.1.29
```

There are also [multiple other installation options](./docs/installation.md).

---

### Usage

To analyze your project sources, run:

```bash
~/phanalist
```

#### Example

![Example](docs/branding/example.gif)

On the first run `phanalist.yaml` will be created with the default configuration and reused on all subsequent runs.

**Additional CLI flags:**

| Flag | Description | Default |
|---|---|---|
| `--config`, `-c` | Path to configuration file | `./phanalist.yaml` |
| `--src`, `-s` | Path(s) to project sources (repeatable, e.g. `-s src -s tests`) | `./src` |
| `--rules`, `-r` | Only run these rules (overrides config) | from config |
| `--output-format`, `-o` | Output format: `text`, `json`, `sarif`, `codeclimate` | `text` |
| `--summary-only` | Show only violation counts per rule | — |
| `--quiet`, `-q` | Suppress all output | — |
| `--verbose`, `-v` | Increase verbosity; repeat for more (`-v` main pass, `-vv` parsing, `-vvv` indexing) | — |
| `--debug-rule-timing` | Print per-rule per-file timing (min/max/avg/p90/p95/p99 + slowest files) | — |
| `--debug-rule-stats` | Print per-rule cost/coverage stats (time, %, violations, files, statements) | — |
| `--use-baseline` | Filter results against a baseline file, reporting only new violations | — |
| `--update-baseline` | Regenerate the baseline from the current scan (requires `--use-baseline`) | — |

---

### Baseline

A baseline lets you adopt phanalist on an existing codebase without fixing every
finding at once. It freezes the current violations; later runs report only new
ones, so CI stays green on known debt but fails on regressions.

Generate (or regenerate) the baseline:

```bash
~/phanalist --use-baseline phanalist-baseline.json --update-baseline
```

Then run against it (in CI, or locally):

```bash
~/phanalist --use-baseline phanalist-baseline.json
```

The baseline is a pretty-printed, stably sorted JSON file, so it produces clean
diffs and merges. Each entry is keyed on the file, rule, and a stable message id
with a count, so unrelated edits that shift line numbers do not invalidate it,
and reworded message text does not either. When you fix violations, regenerate
the baseline to shrink it.

---

### Configuration

```yaml
enabled_rules: []   # empty = all rules active
disable_rules: []
exclude_paths: []   # paths skipped before any rule runs (see below)
rules:
  E0007:
    check_constructor: true
    max_parameters: 5
  E0009:
    max_complexity: 10
  E0010:
    max_paths: 200
  E0012:
    include_namespaces:
      - "App\\Service\\"
      - "App\\Controller\\"
    exclude_namespaces: []
    reset_interfaces:
      - "ResetInterface"
  E0015:
    threshold: 1
  E0016:
    max_complexity: 15
  E0024:
    max_loc: 30
  E0025:
    max_loc: 500
  E0026:
    min_ratio: 0.1
    max_ratio: 0.5
  E0027:
    max_methods: 15
    max_fields: 10
  E0028:
    max_getter_setter_ratio: 0.7
    min_methods: 3
  E0029:
    max_fan_out: 10
    max_fan_in: 20
  E0030:
    max_density: 0.3
```

- **`enabled_rules`** — whitelist of rules to run (empty = all)
- **`disable_rules`** — rules to skip
- **`rules`** — per-rule configuration options
- **`exclude_paths`** — files skipped before any rule runs, as directory prefixes (`var/cache`, `bootstrap/cache`) or globs (`**/*.generated.php`). Handy for framework caches and frozen code like migrations that would only add noise. Literal (non-glob) patterns that don't exist on disk trigger a warning at `-v` verbosity — a helpful catch for typos. Globs that match nothing are silently accepted.

---

### Rules

| Code | Name | Options |
| :--: | :--- | :------ |
| E0000 | Example rule | |
| [E0001](/src/rules/examples/e1/e1.md) | Opening tag position | |
| [E0002](/src/rules/examples/e2/e2.md) | Empty catch | |
| [E0003](/src/rules/examples/e3/e3.md) | Method modifiers | |
| [E0004](/src/rules/examples/e4/e4.md) | Uppercase constants | |
| [E0005](/src/rules/examples/e5/e5.md) | Capitalized class name | |
| [E0006](/src/rules/examples/e6/e6.md) | Property modifiers | |
| [E0007](/src/rules/examples/e7/e7.md) | Method parameters count | `check_constructor: true`, `max_parameters: 5` |
| [E0008](/src/rules/examples/e8/e8.md) | Return type signature | |
| [E0009](/src/rules/examples/e9/e9.md) | Cyclomatic complexity | `max_complexity: 10` |
| [E0010](/src/rules/examples/e10/e10.md) | Npath complexity | `max_paths: 200` |
| [E0011](/src/rules/examples/e11/e11.md) | Detect error suppression symbol (`@`) | |
| [E0012](/src/rules/examples/e12/e12.md) | Service compatibility with Shared Memory Model | `include_namespaces`, `exclude_namespaces`, `reset_interfaces` |
| [E0013](/src/rules/examples/e13/e13.md) | Private method not being used | |
| [E0014](/src/rules/examples/e14/e14.md) | Law of Demeter | |
| [E0015](/src/rules/examples/e15/e15.md) | Lack of Cohesion of Methods (LCOM4) | `threshold: 1` |
| [E0016](/src/rules/examples/e16/e16.md) | Cognitive complexity | `max_complexity: 15` |
| [E0017](/src/rules/examples/e17/e17.md) | Coupling Between Objects (CBO) | `max_coupling: 10` |
| [E0018](/src/rules/examples/e18/e18.md) | Weighted Methods per Class (WMC) | `max_wmc: 50` |
| [E0019](/src/rules/examples/e19/e19.md) | Response For a Class (RFC) | `max_rfc: 50` |
| [E0020](/src/rules/examples/e20/e20.md) | Depth of Inheritance Tree (DIT) | `max_depth: 4` |
| [E0021](/src/rules/examples/e21/e21.md) | Number of Children (NOC) | `max_children: 15` |
| [E0022](/src/rules/examples/e22/e22.md) | Afferent and Efferent Coupling (Ca/Ce) | `max_ca: 20`, `max_ce: 20` |
| [E0023](/src/rules/examples/e23/e23.md) | Instability, Abstractness, Distance (I/A/D) | `max_instability: 0.8`, `max_abstractness: 0.8`, `max_distance: 0.5` |
| [E0024](/src/rules/examples/e24/e24.md) | Lines of Code per Method | `max_loc: 30` |
| [E0025](/src/rules/examples/e25/e25.md) | Lines of Code per File | `max_loc: 500` |
| [E0026](/src/rules/examples/e26/e26.md) | Comment Ratio | `min_ratio: 0.1`, `max_ratio: 0.5` |
| [E0027](/src/rules/examples/e27/e27.md) | God Class (Brain Class) | `max_methods: 15`, `max_fields: 10` |
| [E0028](/src/rules/examples/e28/e28.md) | Data Class | `max_getter_setter_ratio: 0.7`, `min_methods: 3` |
| [E0029](/src/rules/examples/e29/e29.md) | Fan-in / Fan-out | `max_fan_out: 10`, `max_fan_in: 20` |
| [E0030](/src/rules/examples/e30/e30.md) | Cyclomatic Complexity Density | `max_density: 0.3` |

Adding a new rule is straightforward — [this tutorial](./docs/adding_new_rule.md) explains how.

---

### Articles

Read a series of chapters on [https://dev.to/denzyldick](https://dev.to/denzyldick) to understand the project's internals — a great, easy-to-read introduction.

1. [Write your own static analyzer for PHP.](https://dev.to/denzyldick/the-beginning-of-my-php-static-analyzer-in-rust-5bp8)
2. [How I made it impossible to write spaghetti code.](https://dev.to/denzyldick/how-i-made-it-impossible-to-write-spaghetti-code-dg4)
3. [Detecting spaghetti code in AST of a PHP source code.](https://dev.to/denzyldick/traversing-an-ast-of-php-source-code-2kee)
4. [Getting Symfony app ready for Swoole, RoadRunner, and FrankenPHP (no AI involved).](https://dev.to/sergiid/getting-symfony-app-ready-for-swoole-roadrunner-and-frankenphp-no-ai-involved-2d0g)
5. [Improve your CI output](https://dev.to/denzyldick/improve-your-ci-output-2eg)
6. [Why using unserialize in PHP is a bad idea](https://dev.to/denzyldick/why-is-unserializing-an-object-in-php-a-bad-idea-3odl)
