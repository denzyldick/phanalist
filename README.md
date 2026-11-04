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
- [⚙️ Configuration & Rule Sections](#-configuration--rule-sections)
- [📜 Rules Reference](#-rules-reference)
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
- **Formal verification** — checks code paths for common runtime errors (like division by zero or array bounds issues).
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

## ⚙️ Configuration & Rule Sections

Phanalist organizes rules into four internal analysis sections. You can configure thresholds or disable rules in `phanalist.yaml`:

1. **AST & Code Quality (`ast`):** Checks syntax structure, formatting conventions, naming rules, method complexity metrics, and class design.
2. **Formal Verification (`verification`):** Analyzes control flow to prove runtime safety (such as preventing division by zero, null errors, and array bounds violations).
3. **Resource Analysis (`resource`):** Measures time complexity, memory allocation growth, and untrusted loop cardinality.
4. **Security & Lifecycle (`security`):** Detects security vulnerabilities, unclosed file/stream resources, and dangerous execution sinks.

```yaml
enabled_rules: []   # empty = all rules active
disable_rules: []
exclude_paths:
  - vendor
  - node_modules
rules:
  E0007:
    check_constructor: true
    max_parameters: 8
  E0009:
    max_complexity: 10
  E0031:
    php_version: "8.4"
    timeout_ms: 1000
  E0044:
    runtime_threshold: quadratic
```

---

## 📜 Rules Reference

New to the terminology? Check the [Rule Glossary](./docs/rule_glossary.md). Every rule links to detailed, junior-friendly documentation with bad/good examples and junior rules of thumb.

### Section 1: AST & Code Quality Rules (E0000–E0030)
*Type: Syntax, metrics, structure, and design checks.*

| Code | Name | Primary Option / Threshold |
| :--: | :--- | :------ |
| E0000 | Example rule | — |
| [E0001](/src/rules/examples/e1/e1.md) | Opening tag position | — |
| [E0002](/src/rules/examples/e2/e2.md) | Empty catch block | — |
| [E0003](/src/rules/examples/e3/e3.md) | Method visibility modifiers | — |
| [E0004](/src/rules/examples/e4/e4.md) | Uppercase constants | — |
| [E0005](/src/rules/examples/e5/e5.md) | Capitalized class name | — |
| [E0006](/src/rules/examples/e6/e6.md) | Property visibility modifiers | — |
| [E0007](/src/rules/examples/e7/e7.md) | Method parameters count | `check_constructor`, `max_parameters: 8` |
| [E0008](/src/rules/examples/e8/e8.md) | Return type signature | — |
| [E0009](/src/rules/examples/e9/e9.md) | Cyclomatic complexity | `max_complexity: 10` |
| [E0010](/src/rules/examples/e10/e10.md) | Npath complexity | `max_paths: 200` |
| [E0011](/src/rules/examples/e11/e11.md) | Error suppression symbol (`@`) | — |
| [E0012](/src/rules/examples/e12/e12.md) | Service Shared Memory Model compatibility | `include_namespaces`, `reset_interfaces` |
| [E0013](/src/rules/examples/e13/e13.md) | Unused private method | — |
| [E0014](/src/rules/examples/e14/e14.md) | Law of Demeter | — |
| [E0015](/src/rules/examples/e15/e15.md) | Method Cohesion (LCOM4) | `threshold: 1` |
| [E0016](/src/rules/examples/e16/e16.md) | Cognitive complexity | `max_complexity: 15` |
| [E0017](/src/rules/examples/e17/e17.md) | Coupling Between Objects (CBO) | `max_coupling: 10` |
| [E0018](/src/rules/examples/e18/e18.md) | Weighted Methods per Class (WMC) | `max_wmc: 50` |
| [E0019](/src/rules/examples/e19/e19.md) | Response For a Class (RFC) | `max_rfc: 50` |
| [E0020](/src/rules/examples/e20/e20.md) | Depth of Inheritance Tree (DIT) | `max_depth: 4` |
| [E0021](/src/rules/examples/e21/e21.md) | Number of Children (NOC) | `max_children: 15` |
| [E0022](/src/rules/examples/e22/e22.md) | Afferent/Efferent Coupling (Ca/Ce) | `max_ca: 20`, `max_ce: 20` |
| [E0023](/src/rules/examples/e23/e23.md) | Instability, Abstractness, Distance | `max_distance: 0.5` |
| [E0024](/src/rules/examples/e24/e24.md) | Lines of Code per Method | `max_loc: 50` |
| [E0025](/src/rules/examples/e25/e25.md) | Lines of Code per File | `max_loc: 500` |
| [E0026](/src/rules/examples/e26/e26.md) | Comment Ratio | `min_ratio: 0.05`, `max_ratio: 0.5` |
| [E0027](/src/rules/examples/e27/e27.md) | God Class (Brain Class) | `max_methods: 15`, `max_fields: 10` |
| [E0028](/src/rules/examples/e28/e28.md) | Data Class | `min_methods: 3` |
| [E0029](/src/rules/examples/e29/e29.md) | Fan-in / Fan-out | `max_fan_out: 10`, `max_fan_in: 20` |
| [E0030](/src/rules/examples/e30/e30.md) | Cyclomatic Complexity Density | `max_density: 0.3` |

### Section 2: Formal Verification Rules (E0031–E0043)
*Type: Path analysis and runtime safety checks.*

| Code | Name | What it Proves / Checks |
| :--: | :--- | :------ |
| [E0031](/src/rules/examples/e31/e31.md) | Division by Zero | Proves that divisors are never zero |
| [E0032](/src/rules/examples/e32/e32.md) | Modulo by Zero | Proves that modulo divisors are never zero |
| [E0033](/src/rules/examples/e33/e33.md) | `intdiv()` by Zero | Proves integer division divisors are valid |
| [E0034](/src/rules/examples/e34/e34.md) | `intdiv()` Overflow | Prevents integer overflow during division |
| [E0035](/src/rules/examples/e35/e35.md) | Missing Array Key | Proves that required array keys exist before access |
| [E0036](/src/rules/examples/e36/e36.md) | Array Index Out of Bounds | Proves array index access stays within bounds |
| [E0037](/src/rules/examples/e37/e37.md) | String Offset Out of Bounds | Proves string character offsets are valid |
| [E0038](/src/rules/examples/e38/e38.md) | Method Call on Null | Proves objects are not null before calling methods |
| [E0039](/src/rules/examples/e39/e39.md) | Property Access on Null | Proves objects are not null before accessing properties |
| [E0040](/src/rules/examples/e40/e40.md) | Return Contract Violation | Verifies return types match documented contracts |
| [E0041](/src/rules/examples/e41/e41.md) | Unreachable Code | Detects dead statements after returns or throws |
| [E0042](/src/rules/examples/e42/e42.md) | Impossible Assertion | Detects assertions that can never hold true |
| [E0043](/src/rules/examples/e43/e43.md) | Stream Resource Used After Close | Detects reading/writing streams after closing them |

### Section 3: Resource Analysis Rules (E0044–E0046)
*Type: Scalability, memory growth, and time complexity checks.*

| Code | Name | What it Analyzes |
| :--: | :--- | :------ |
| [E0044](/src/rules/examples/e44/e44.md) | Input-Dependent Runtime Growth | Flags loops whose time complexity scales poorly with input size |
| [E0045](/src/rules/examples/e45/e45.md) | Input-Dependent Live-Heap Growth | Flags memory allocation patterns that grow with input size |
| [E0046](/src/rules/examples/e46/e46.md) | Input-Controlled Cardinality | Flags collections sized directly from untrusted user input |

### Section 4: Security & Lifecycle Rules (E0047–E0059)
*Type: Security hazards and resource leak checks.*

| Code | Name | Security / Lifecycle Risk |
| :--: | :--- | :------ |
| [E0047](/src/rules/examples/e47/e47.md) | Resource Lifecycle Leak | Unclosed file handles or database connections |
| [E0048](/src/rules/examples/e48/e48.md) | Unbounded Direct Recursion | Infinite recursion without base condition changes |
| [E0049](/src/rules/examples/e49/e49.md) | Builtin Argument Validation | Invalid literal arguments passed to built-in functions |
| [E0050](/src/rules/examples/e50/e50.md) | Position Check Used as Boolean | Using `strpos()` without strict comparison (`!== false`) |
| [E0051](/src/rules/examples/e51/e51.md) | Count of Non-Countable Value | Calling `count()` on non-array scalars |
| [E0052](/src/rules/examples/e52/e52.md) | Unchecked `json_decode` Result | Missing validation after JSON decoding |
| [E0053](/src/rules/examples/e53/e53.md) | Unserialize Without `allowed_classes` | PHP object injection risk |
| [E0054](/src/rules/examples/e54/e54.md) | Dynamic Variable Creation | Variable variables (`$$x`) or `extract()` usage |
| [E0055](/src/rules/examples/e55/e55.md) | Dynamic Include or Require | Unsafe dynamic file inclusion paths |
| [E0056](/src/rules/examples/e56/e56.md) | Weak Randomness for Security | Using `rand()` or `mt_rand()` for security tokens |
| [E0057](/src/rules/examples/e57/e57.md) | MD5 or SHA1 for Passwords | Insecure hashing algorithms for credentials |
| [E0058](/src/rules/examples/e58/e58.md) | Cookie Missing Secure or HttpOnly | Cookies missing security flags |
| [E0059](/src/rules/examples/e59/e59.md) | Dynamic Eval or Assert | Dangerous code execution sinks (`eval()`) |

Adding a new rule is straightforward — [this tutorial](./docs/adding_new_rule.md) explains how.

---

### 🔌 Language Server Protocol (LSP)

Phanalist includes built-in LSP support to provide real-time, as-you-type diagnostics directly in your editor (`phanalist --lsp`).

---

### Articles

Read a series of chapters on [https://dev.to/denzyldick](https://dev.to/denzyldick) to understand the project's internals.
