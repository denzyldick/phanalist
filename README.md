<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/docs/branding/banner.png" alt="Phanalist banner" width="100%"/>

# Phanalist

> Fast static analyzer for PHP. Finds maintainability and runtime risks with zero configuration required.

[![Build](https://img.shields.io/github/actions/workflow/status/denzyldick/phanalist/release.yml?branch=main&label=build)](https://github.com/denzyldick/phanalist/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/denzyldick/phanalist)](https://github.com/denzyldick/phanalist/releases/latest)
[![License](https://img.shields.io/github/license/denzyldick/phanalist)](LICENSE)

---

## What it does

Phanalist scans PHP codebases and reports actionable findings — no configuration required. Each finding explains what's wrong, why it matters, and how to fix it.

It covers:

- **Code health** — class design, complexity, coupling, method size, naming
- **Runtime risk** — division by zero, array/string bounds, null dereferences, unreachable code
- **Resource usage** — loops and allocations that grow with input
- **Security** — SQL injection, XSS, command injection, weak crypto, unsafe deserialization, header/mail injection, and more

## Quickstart

Install:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/denzyldick/phanalist/main/bin/init.sh | sh
```

Scan your project:

```bash
cd your-php-project
~/phanalist
```

On first run a `phanalist.yaml` is created with sensible defaults. Review findings, then integrate into CI:

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

See [Installation](./docs/installation.md) for Composer, Docker, and source-build options.

## Features

- **Fast** — built in Rust, analyzes large codebases in seconds
- **136 built-in rules** across code health, runtime verification, resource usage, and security
- **Zero config** — works out of the box; tune via `phanalist.yaml`
- **Multiple output formats** — text, JSON, SARIF (GitHub code scanning), Code Climate
- **Editor integration** — built-in Language Server (`phanalist --lsp`)
- **Engineer quality report** — attribute violations to authors via `git blame` (`--blame`)
- **Baseline** — adopt on legacy codebases without fixing everything at once (`--use-baseline`)

## Configuration

Tune rules in `phanalist.yaml`:

```yaml
enabled_rules: []      # empty = all rules active
disable_rules: []
exclude_paths:
  - vendor
  - node_modules
rules:
  E0007:
    max_parameters: 8
  E0009:
    max_complexity: 10
```

All rules are enabled by default; use `enabled_rules`, `disable_rules`, and per-rule `rules` settings in `phanalist.yaml` to customize them:

1. **AST & Code Quality:** Checks syntax structure, formatting conventions, naming rules, method complexity metrics, and class design.
2. **Formal Verification:** Analyzes control flow to prove runtime safety (such as preventing division by zero, null errors, and array bounds violations).
3. **Resource Analysis:** Measures time complexity, memory allocation growth, and untrusted loop cardinality.
4. **Lifecycle & Runtime Safety:** Checks resource cleanup, runtime-error traps, and reliability hazards.
5. **Security:** Detects security vulnerabilities and dangerous execution sinks.

Security rules that use source-to-sink taint analysis accept a per-rule `taint_mode`:

- `annotate` (default) keeps the finding and adds the proven source origin.
- `only` reports only proven tainted flows.
- `off` keeps the legacy heuristic-only behaviour.

## Rules

Phanalist organizes rules into four sections:

| Section | Codes | What it checks |
| :--- | :--- | :--- |
| Code quality | E0000–E0030 | Syntax, naming, metrics, class design |
| Formal verification | E0031–E0043 | Division/bounds/null safety, unreachable code, contracts |
| Resource analysis | E0044–E0046 | Input-dependent loops, memory growth, cardinality |
| Security & lifecycle | E0047–E0135 | Injection, weak crypto, unsafe deserialization, headers, cookies |

Every rule links to junior-friendly documentation with bad/good examples, "why it matters," and fix instructions:

- [Rules reference & details](src/rules/examples/)
- [Rule glossary](./docs/rule_glossary.md)
- [Adding a new rule](./docs/adding_new_rule.md)

### Section 4: Lifecycle & Runtime Safety Rules (E0047–E0052)
*Type: Resource lifecycle, runtime-correctness, and reliability checks.*

| Code | Name | What it Checks |
| :--: | :--- | :------ |
| [E0047](/src/rules/examples/e47/e47.md) | Resource Lifecycle Leak | Unclosed file handles or database connections |
| [E0048](/src/rules/examples/e48/e48.md) | Unbounded Direct Recursion | Infinite recursion without base condition changes |
| [E0049](/src/rules/examples/e49/e49.md) | Builtin Argument Validation | Invalid literal arguments passed to built-in functions |
| [E0050](/src/rules/examples/e50/e50.md) | Position Check Used as Boolean | Using `strpos()` without strict comparison (`!== false`) |
| [E0051](/src/rules/examples/e51/e51.md) | Count of Non-Countable Value | Calling `count()` on non-array scalars |
| [E0052](/src/rules/examples/e52/e52.md) | Unchecked `json_decode` Result | Missing validation after JSON decoding |

### Section 5: Security Rules (E0053–E0135)
*Type: Security vulnerabilities and dangerous sinks.*

| Code | Name | Security Risk |
| :--: | :--- | :------ |
| [E0053](/src/rules/examples/e53/e53.md) | Unserialize Without `allowed_classes` | PHP object injection risk |
| [E0054](/src/rules/examples/e54/e54.md) | Dynamic Variable Creation | Variable variables (`$$x`) or `extract()` usage |
| [E0055](/src/rules/examples/e55/e55.md) | Dynamic Include or Require | Unsafe dynamic file inclusion paths |
| [E0056](/src/rules/examples/e56/e56.md) | Weak Randomness for Security | Using `rand()` or `mt_rand()` for security tokens |
| [E0057](/src/rules/examples/e57/e57.md) | MD5 or SHA1 for Passwords | Insecure hashing algorithms for credentials |
| [E0058](/src/rules/examples/e58/e58.md) | Cookie Missing Secure or HttpOnly | Cookies missing security flags |
| [E0059](/src/rules/examples/e59/e59.md) | Dynamic Eval or Assert | Dangerous code execution sinks (`eval()`) |
| [E0060–E0135](/src/rules/examples/) | Additional security rules | SQL injection, XSS, command injection, LDAP/XPath/NoSQL injection, header/mail injection, unsafe deserialization, file inclusion/upload, path traversal, SSRF, open redirect, XXE, template injection, regex DoS, weak crypto, information disclosure |

## Editor integration

Run Phanalist as a Language Server for real-time diagnostics:

```bash
phanalist --lsp
```

## Community

- [Contributing](./CONTRIBUTING.md)
- [Code of Conduct](./CODE_OF_CONDUCT.md)
- [Security policy](./SECURITY.md)
- Articles: [dev.to/denzyldick](https://dev.to/denzyldick)

MIT Licensed.
