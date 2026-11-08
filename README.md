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

See [Configuration](./docs/installation.md) for the full reference.

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
