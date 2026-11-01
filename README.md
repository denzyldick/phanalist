<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/docs/branding/banner-cropped.png"/>

> Fast static analyzer for PHP, written in Rust. Finds maintainability and runtime risks with zero configuration required.

---

## 📖 Table of Contents
- [Why Phanalist?](#-why-phanalist)
- [✨ Features](#-features)
- [🚀 Quickstart (5 Minutes)](#-quickstart-5-minutes)
- [📦 Installation](./docs/installation.md)
- [💻 Usage & CLI Flags](#-usage)
- [📈 Engineer Quality Report (--blame)](#-engineer-quality-report)
- [🛡️ Baseline (Adopting without fixing everything)](#-baseline)
- [⚙️ Configuration](#-configuration)
- [📜 Rules Reference](#-rules)
- [🔌 Language Server Protocol (LSP)](#-language-server-protocol-lsp)

---

## 🤔 Why Phanalist?

PHP codebases grow. As they grow, they collect technical debt: classes that do too much, methods that are hard to follow, and hidden risks that only appear under real traffic. Most tools check syntax, style, or types. Phanalist checks whether the code will stay **maintainable and safe as it grows**.

It looks at two core things:
- **Code health** — complexity, coupling, class design, and readability
- **Runtime risk** — memory that grows with input, work that grows too fast, and collections with no size limit

In plain terms, Phanalist helps you answer questions like:
- Is this class becoming too large to maintain?
- Does this loop keep allocating memory as input grows?
- Is this code using a risky PHP feature in a security-sensitive place?

Each finding explains what is wrong, why it matters, and how to fix it. **You do not need to be a static-analysis expert to use it.**

---

## ✨ Features

- 🚀 **Fast** — built in Rust, analyzes large codebases in seconds and keeps memory use predictable.
- 🔍 **59 built-in rules** — covering architecture, runtime risk, security, and code health.
- **Formal verification** — uses a solver (Z3) to check every modeled path for common runtime errors (like division by zero or array bounds issues).
- **Resource analysis** — finds loops and functions that use more time or memory as input grows.
- **Security checks** — flags risky PHP patterns such as `eval()`, unsafe deserialization (`unserialize`), weak randomness, and insecure cookies.
- ⚙️ **Zero config to start** — works out of the box; configure only what you need in `phanalist.yaml`.
- 📄 **Multiple output formats** — `text`, `json`, `sarif` for GitHub code scanning, and `codeclimate` for code-quality platforms.
- 🔌 **Extensible** — adding a custom rule takes minutes.

---

## 🚀 Quickstart (5 Minutes)

### Step 1: Install Phanalist
Run the installation script in your terminal:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/denzyldick/phanalist/main/bin/init.sh | sh
```

### Step 2: Run Your First Scan
Navigate to your PHP project root and run:
```bash
~/phanalist
```
*(On the first run, `phanalist.yaml` is automatically created with sensible defaults).*

### Step 3: Review Findings
Phanalist outputs actionable findings with file paths, line numbers, and clear explanations. To see violations per rule summary only, run:
```bash
~/phanalist --summary-only
```

---

### Where Phanalist Fits in Your Pipeline

| Tool category | What it usually checks | What Phanalist adds | Run together? |
|---|---|---|---|
| Type analysis | Types, nullability, and contracts | Architecture, runtime/resource, and security findings | Yes |
| Style and formatting | Coding standard and layout | Design and runtime risk | Yes |
| Architecture and metrics | Coupling, complexity, and dependency rules | Actionable findings with explanations and fix guidance | Yes, or use instead |
| Security scanning | Broad source-to-sink analysis | Targeted PHP-specific hazards | Yes |
| Quality dashboard | History, trends, and team reporting | SARIF and Code Climate output for your existing dashboard | Yes |

---

### GitHub Action

Add inline SARIF annotations to your PRs using the [Phanalist GitHub Action](https://github.com/marketplace/actions/phanalist):

```yaml
name: Phanalist
on: [pull_request]
permissions:
  security-events: write
jobs:
  phanalist:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: denzyldick/phanalist-action@v1
```

By default, the action appends each rule's full documentation to inline pull request annotations.

---

### Usage

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
| `--debug-rule-timing` | Print per-rule per-file timing | — |
| `--debug-rule-stats` | Print per-rule cost/coverage stats | — |
| `--use-baseline` | Filter results against a baseline file, reporting only new violations | — |
| `--update-baseline` | Regenerate the baseline from the current scan | — |
| `--blame` | Attribute violations to engineers via git blame and show a quality report | — |
| `--since` | Only count violations from commits after this date (e.g. `"30 days"`, `"2025-01-01"`) | — |
| `--until` | Only count violations from commits before this date | — |
| `--export-chart` | Export engineer chart as PNG/SVG image (requires `--blame`) | — |
| `--exclude-author` | Exclude authors from the report (repeatable, e.g. `--exclude-author dependabot`) | — |
| `--min-violations` | Minimum total violations to include an engineer in the report | `0` |
| `--lsp` | Start as a Language Server (LSP) for editor integrations | — |

---

### Engineer Quality Report

Use the `--blame` flag to see who is introducing and fixing violations in your codebase:

```bash
# Who has violations in their code currently
~/phanalist --blame

# See what changed in the last 30 days (Net column shows who is improving vs degrading)
~/phanalist --blame --since "30 days"

# Exclude bots and sort by net improvements
~/phanalist --blame --since "30 days" --exclude-author dependabot --sort net
```

| Column | Meaning |
|---|---|
| **Engineer** | Git author name |
| **Fixed (✓)** | Violations that disappeared compared to the `--since` snapshot |
| **Introduced (✗)** | Violations that appeared compared to the `--since` snapshot |
| **Net** | `Fixed - Introduced` — positive means improving, negative means degrading |

---

### Baseline

A baseline lets you adopt Phanalist on an existing codebase without fixing every finding at once. It freezes current violations; later runs report only new ones so CI stays green on known debt but fails on regressions.

```bash
# Generate baseline
~/phanalist --use-baseline phanalist-baseline.json --update-baseline

# Run against baseline in CI
~/phanalist --use-baseline phanalist-baseline.json
```

---

### Configuration

```yaml
enabled_rules: []   # empty = all rules active
disable_rules: []
exclude_paths:
  - vendor
  - node_modules
  - var/generated
rules:
  E0007:
    check_constructor: true
    max_parameters: 5
  E0009:
    max_complexity: 10
  # ... (see documentation for individual rule settings)
```

---

### Rules

New to the terminology? Check the [Rule Glossary](./docs/rule_glossary.md). Every rule links to detailed, junior-friendly documentation with bad/good examples and junior rules of thumb.

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
| [E0031](/src/rules/examples/e31/e31.md) | Division by Zero | Configurable |
| [E0032](/src/rules/examples/e32/e32.md) | Modulo by Zero | Configurable |
| [E0033](/src/rules/examples/e33/e33.md) | `intdiv()` by Zero | Configurable |
| [E0034](/src/rules/examples/e34/e34.md) | `intdiv()` Overflow | Configurable |
| [E0035](/src/rules/examples/e35/e35.md) | Missing Array Key | Configurable |
| [E0036](/src/rules/examples/e36/e36.md) | Array Index Out of Bounds | Configurable |
| [E0037](/src/rules/examples/e37/e37.md) | String Offset Out of Bounds | Configurable |
| [E0038](/src/rules/examples/e38/e38.md) | Method Call on Null | Configurable |
| [E0039](/src/rules/examples/e39/e39.md) | Property Access on Null | Configurable |
| [E0040](/src/rules/examples/e40/e40.md) | Return Contract Violation | Configurable |
| [E0041](/src/rules/examples/e41/e41.md) | Unreachable Code | Configurable |
| [E0042](/src/rules/examples/e42/e42.md) | Impossible Assertion | Configurable |
| [E0043](/src/rules/examples/e43/e43.md) | Stream Resource Used After Close | Configurable |
| [E0044](/src/rules/examples/e44/e44.md) | Input-dependent runtime growth | Configurable |
| [E0045](/src/rules/examples/e45/e45.md) | Input-dependent live-heap growth | Configurable |
| [E0046](/src/rules/examples/e46/e46.md) | Input-controlled cardinality | Configurable |
| [E0047](/src/rules/examples/e47/e47.md) | Resource lifecycle leak | |
| [E0048](/src/rules/examples/e48/e48.md) | Unbounded direct recursion | |
| [E0049](/src/rules/examples/e49/e49.md) | Builtin argument validation | |
| [E0050](/src/rules/examples/e50/e50.md) | Position check used as boolean | |
| [E0051](/src/rules/examples/e51/e51.md) | Count of a non-countable value | |
| [E0052](/src/rules/examples/e52/e52.md) | Unchecked json_decode result | |
| [E0053](/src/rules/examples/e53/e53.md) | Unserialize without allowed_classes | |
| [E0054](/src/rules/examples/e54/e54.md) | Dynamic variable creation | |
| [E0055](/src/rules/examples/e55/e55.md) | Dynamic include or require | |
| [E0056](/src/rules/examples/e56/e56.md) | Weak randomness for security | |
| [E0057](/src/rules/examples/e57/e57.md) | MD5 or SHA1 for passwords | |
| [E0058](/src/rules/examples/e58/e58.md) | Cookie missing Secure or HttpOnly | |
| [E0059](/src/rules/examples/e59/e59.md) | Dynamic eval or assert | |

Adding a new rule is straightforward — [this tutorial](./docs/adding_new_rule.md) explains how.

---

### 🔌 Language Server Protocol (LSP)

Phanalist includes built-in LSP support to provide real-time, as-you-type diagnostics directly in your editor (`phanalist --lsp`).

---

### Articles

Read a series of chapters on [https://dev.to/denzyldick](https://dev.to/denzyldick) to understand the project's internals.
