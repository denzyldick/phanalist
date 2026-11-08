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

---

## 📜 Rules Reference

Every rule links to junior-friendly documentation with bad/good examples, "why it matters," and fix instructions.

### Section 1: AST & Code Quality Rules (E0000–E0030)
*Type: Syntax, metrics, structure, and design checks.*

| Code | Name | Primary Option / Threshold |
| :--: | :--- | :--- |
| [E0000](/src/rules/examples/e0/e0.md) | Example rule | — |
| [E0001](/src/rules/examples/e1/e1.md) | Opening tag position | — |
| [E0002](/src/rules/examples/e2/e2.md) | Empty catch block | — |
| [E0003](/src/rules/examples/e3/e3.md) | Method modifiers | — |
| [E0004](/src/rules/examples/e4/e4.md) | Uppercase constants | — |
| [E0005](/src/rules/examples/e5/e5.md) | Capitalized class name | — |
| [E0006](/src/rules/examples/e6/e6.md) | Property visibility modifiers | — |
| [E0007](/src/rules/examples/e7/e7.md) | Method parameters count | `max_parameters: 8` |
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
| :--: | :--- | :--- |
| [E0031](/src/rules/examples/e31/e31.md) | Division by Zero | Proves that divisors are never zero |
| [E0032](/src/rules/examples/e32/e32.md) | Modulo by Zero | Proves that modulo divisors are never zero |
| [E0033](/src/rules/examples/e33/e33.md) | `intdiv()` by Zero | Proves integer division divisors are valid |
| [E0034](/src/rules/examples/e34/e34.md) | `intdiv()` Overflow | Prevents integer overflow during division |
| [E0035](/src/rules/examples/e35/e35.md) | Missing Array Key | Proves required array keys exist before access |
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
| :--: | :--- | :--- |
| [E0044](/src/rules/examples/e44/e44.md) | Input-Dependent Runtime Growth | Flags loops whose time complexity scales poorly with input size |
| [E0045](/src/rules/examples/e45/e45.md) | Input-Dependent Live-Heap Growth | Flags memory allocation patterns that grow with input size |
| [E0046](/src/rules/examples/e46/e46.md) | Input-Controlled Cardinality | Flags collections sized directly from untrusted user input |

### Section 4: Security & Lifecycle Rules (E0047–E0135)
*Type: Security vulnerabilities and dangerous execution sinks.*

#### E0047–E0052 — Resource lifecycle & runtime safety

| Code | Name | Risk |
| :--: | :--- | :--- |
| [E0047](/src/rules/examples/e47/e47.md) | Resource Lifecycle Leak | Unclosed file handles or database connections |
| [E0048](/src/rules/examples/e48/e48.md) | Unbounded Direct Recursion | Infinite recursion without base condition changes |
| [E0049](/src/rules/examples/e49/e49.md) | Builtin Argument Validation | Invalid literal arguments passed to built-in functions |
| [E0050](/src/rules/examples/e50/e50.md) | Position Check Used as Boolean | Using `strpos()` without strict comparison (`!== false`) |
| [E0051](/src/rules/examples/e51/e51.md) | Count of Non-Countable Value | Calling `count()` on non-array scalars |
| [E0052](/src/rules/examples/e52/e52.md) | Unchecked `json_decode` Result | Missing validation after JSON decoding |

#### E0053–E0059 — Security hazards

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0053](/src/rules/examples/e53/e53.md) | Unserialize Without `allowed_classes` | PHP object injection risk |
| [E0054](/src/rules/examples/e54/e54.md) | Dynamic Variable Creation | Variable variables (`$$x`) or `extract()` usage |
| [E0055](/src/rules/examples/e55/e55.md) | Dynamic Include or Require | Unsafe dynamic file inclusion paths |
| [E0056](/src/rules/examples/e56/e56.md) | Weak Randomness for Security | Using `rand()` or `mt_rand()` for security tokens |
| [E0057](/src/rules/examples/e57/e57.md) | MD5 or SHA1 for Passwords | Insecure hashing algorithms for credentials |
| [E0058](/src/rules/examples/e58/e58.md) | Cookie Missing Secure or HttpOnly | Cookies missing security flags |
| [E0059](/src/rules/examples/e59/e59.md) | Dynamic Eval or Assert | Dangerous code execution sinks (`eval()`) |

#### E0060–E0069 — Injection attacks

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0060](/src/rules/examples/e60/e60.md) | SQL Injection | User input concatenated into SQL queries |
| [E0061](/src/rules/examples/e61/e61.md) | Cross-Site Scripting (XSS) | Unescaped user input echoed to the browser |
| [E0062](/src/rules/examples/e62/e62.md) | Command Injection | User input passed to shell execution functions |
| [E0063](/src/rules/examples/e63/e63.md) | LDAP Injection | User input embedded in LDAP filters |
| [E0064](/src/rules/examples/e64/e64.md) | XPath Injection | User input embedded in XPath queries |
| [E0065](/src/rules/examples/e65/e65.md) | NoSQL Injection | User input embedded in NoSQL queries |
| [E0066](/src/rules/examples/e66/e66.md) | Header Injection | User input written to HTTP headers (CRLF injection) |
| [E0067](/src/rules/examples/e67/e67.md) | Mail Injection | User input written to mail headers or subject |
| [E0068](/src/rules/examples/e68/e68.md) | Eval Injection | User input evaluated as PHP code via `eval()` |
| [E0069](/src/rules/examples/e69/e69.md) | Unsafe Deserialization | Untrusted data passed to `unserialize()` |

#### E0070–E0079 — File, session, and crypto risks

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0070](/src/rules/examples/e70/e70.md) | Remote File Inclusion | User-controlled file included via `include`/`require` |
| [E0071](/src/rules/examples/e71/e71.md) | Unsafe File Upload | Uploaded file stored to user-controlled destination |
| [E0072](/src/rules/examples/e72/e72.md) | Path Traversal | User-controlled file path opened for read/write |
| [E0073](/src/rules/examples/e73/e73.md) | Weak Cryptographic Hash | MD5/SHA1 used instead of `password_hash()` |
| [E0074](/src/rules/examples/e74/e74.md) | Weak Random Number Generator | `mt_rand()`/`rand()` used for security values |
| [E0075](/src/rules/examples/e75/e75.md) | Session Security | `session_start()` without secure cookie settings |
| [E0076](/src/rules/examples/e76/e76.md) | Information Disclosure | `phpinfo()` left in production code |
| [E0077](/src/rules/examples/e77/e77.md) | Display Errors Enabled | `display_errors` enabled via `ini_set()` |
| [E0078](/src/rules/examples/e78/e78.md) | Variable Variables | `extract()` importing user-controlled variables |
| [E0079](/src/rules/examples/e79/e79.md) | Dynamic Method Call | Dynamic method invoked on user-controlled object |

#### E0080–E0089 — Deprecated functions & injection

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0080](/src/rules/examples/e80/e80.md) | Weak Cryptographic Function | `crypt()` used for password hashing |
| [E0081](/src/rules/examples/e81/e81.md) | Deprecated Regex Function | `ereg`/`split` used instead of `preg_*` |
| [E0082](/src/rules/examples/e82/e82.md) | Dynamic Extension Loading | `dl()` loading extensions at runtime |
| [E0083](/src/rules/examples/e83/e83.md) | Open Redirect | `header()` redirect to user-controlled URL |
| [E0084](/src/rules/examples/e84/e84.md) | SQL Query String Concatenation | SQL query built by concatenating user input |
| [E0085](/src/rules/examples/e85/e85.md) | Inline PHP Execution Short Tags | `<?= ?>` short echo tag usage |
| [E0086](/src/rules/examples/e86/e86.md) | Deprecated Session Function | `session_register()`/`session_is_registered()` usage |
| [E0087](/src/rules/examples/e87/e87.md) | Command Execution with Backticks | Backtick command with interpolated input |
| [E0088](/src/rules/examples/e88/e88.md) | Reflection-Based Injection | Reflection invoked on user-controlled input |
| [E0089](/src/rules/examples/e89/e89.md) | Log Injection | User input written to log files unsanitized |

#### E0090–E0099 — File, XML, and cookie security

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0090](/src/rules/examples/e90/e90.md) | File Write to User-Controlled Path | `file_put_contents()` to user-controlled path |
| [E0091](/src/rules/examples/e91/e91.md) | Source Code Exposure | `highlight_file()`/`show_source()` in production |
| [E0092](/src/rules/examples/e92/e92.md) | XML External Entity (XXE) | XML parsed from user-controlled input |
| [E0093](/src/rules/examples/e93/e93.md) | Template Injection | User input rendered as a template |
| [E0094](/src/rules/examples/e94/e94.md) | Denial of Service via Regex | Regex applied to user-controlled input |
| [E0095](/src/rules/examples/e95/e95.md) | Insecure Comparison | `strcmp()`/`strcasecmp()` not constant-time |
| [E0096](/src/rules/examples/e96/e96.md) | Cookie Without Secure Flag | Cookie set without the Secure flag |
| [E0097](/src/rules/examples/e97/e97.md) | Cookie Without HttpOnly Flag | Cookie set without the HttpOnly flag |
| [E0098](/src/rules/examples/e98/e98.md) | Session ID in URL | Session ID embedded in redirect URL |
| [E0099](/src/rules/examples/e99/e99.md) | Disable Functions Bypass | `ini_set()` changing `disable_functions` |

#### E0100–E0109 — Dynamic code & SSRF

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0100](/src/rules/examples/e100/e100.md) | Variable Function Call | Variable function invoked on user-controlled name |
| [E0101](/src/rules/examples/e101/e101.md) | Object Injection via `unserialize` | Object injection from unserialize of user input |
| [E0102](/src/rules/examples/e102/e102.md) | Command Injection via `proc_open` | `proc_open()` with user-controlled command |
| [E0103](/src/rules/examples/e103/e103.md) | Server-Side Request Forgery | Network request to user-controlled URL |
| [E0104](/src/rules/examples/e104/e104.md) | HTTP Response Splitting | `header()` with user-controlled CRLF sequences |
| [E0105](/src/rules/examples/e105/e105.md) | Local File Inclusion | Local file included from user-controlled input |
| [E0106](/src/rules/examples/e106/e106.md) | Deserialization of Untrusted Data | `unserialize()`/`wddx_deserialize()` of user input |
| [E0107](/src/rules/examples/e107/e107.md) | Weak Random for Security Tokens | Weak random used for security-sensitive tokens |
| [E0108](/src/rules/examples/e108/e108.md) | Dynamic Instantiation | Class instantiated from user-controlled name |
| [E0109](/src/rules/examples/e109/e109.md) | Directory Listing Exposure | `opendir()` on user-controlled directory |

#### E0110–E0119 — SQL, XML, and session security

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0110](/src/rules/examples/e110/e110.md) | Raw SQL Query Execution | Raw SQL query executed without parameters |
| [E0111](/src/rules/examples/e111/e111.md) | Deprecated MySQL Extension | Removed `ext/mysql` extension functions used |
| [E0112](/src/rules/examples/e112/e112.md) | File Upload Without Validation | `move_uploaded_file()` without destination validation |
| [E0113](/src/rules/examples/e113/e113.md) | Session Regeneration Missing | Session ID not regenerated after authentication |
| [E0114](/src/rules/examples/e114/e114.md) | PHP Info Disclosure | `phpinfo()` exposing server configuration |
| [E0115](/src/rules/examples/e115/e115.md) | XXE via libxml | XML parsed by libxml from user-controlled input |
| [E0116](/src/rules/examples/e116/e116.md) | Remote Command Execution via `popen` | `popen()` with user-controlled command |
| [E0117](/src/rules/examples/e117/e117.md) | SQL Injection via Query String | SQL query string built from user input |
| [E0118](/src/rules/examples/e118/e118.md) | Log Forging via User Input | User input written to logs without sanitization |
| [E0119](/src/rules/examples/e119/e119.md) | Session Cookie Without Secure Flag | Session cookie set without Secure flag |

#### E0120–E0135 — Dynamic execution & header injection

| Code | Name | Security Risk |
| :--: | :--- | :--- |
| [E0120](/src/rules/examples/e120/e120.md) | Eval With User-Controlled String | `eval()` executed on user-controlled string |
| [E0121](/src/rules/examples/e121/e121.md) | Assert With User-Controlled String | `assert()` with user-controlled string argument |
| [E0122](/src/rules/examples/e122/e122.md) | File Inclusion With User Input | `include`/`require` with user-controlled path |
| [E0123](/src/rules/examples/e123/e123.md) | XSS via `echo` | `echo()` outputs user-controlled input unescaped |
| [E0124](/src/rules/examples/e124/e124.md) | Weak Hash for Passwords | Weak hash (MD5/SHA1/crypt) used for passwords |
| [E0125](/src/rules/examples/e125/e125.md) | Open Redirect via `header` | `header()` redirects to user-controlled URL |
| [E0126](/src/rules/examples/e126/e126.md) | Path Traversal via `file_get_contents` | `file_get_contents()` reads user-controlled path |
| [E0127](/src/rules/examples/e127/e127.md) | Command Execution via `system` | `system()` with user-controlled command |
| [E0128](/src/rules/examples/e128/e128.md) | Command Execution via `exec` | `exec()` with user-controlled command |
| [E0129](/src/rules/examples/e129/e129.md) | Command Execution via `shell_exec` | `shell_exec()` with user-controlled command |
| [E0130](/src/rules/examples/e130/e130.md) | Command Execution via `passthru` | `passthru()` with user-controlled command |
| [E0131](/src/rules/examples/e131/e131.md) | Command Execution via `proc_open` | `proc_open()` with user-controlled command |
| [E0132](/src/rules/examples/e132/e132.md) | Header Injection via `header` | `header()` writes user input to HTTP headers |
| [E0133](/src/rules/examples/e133/e133.md) | Header Injection via `setcookie` | `setcookie()` writes user input to cookie header |
| [E0134](/src/rules/examples/e134/e134.md) | Mail Injection via `mail` | `mail()` writes user input to mail headers/subject |
| [E0135](/src/rules/examples/e135/e135.md) | PHP Info Disclosure | `phpinfo()` exposing configuration details |

---

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
