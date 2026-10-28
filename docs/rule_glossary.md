# Rule Glossary

Plain-English definitions for terms used in Phanalist rule documentation.

## PHP basics

**Truthy / falsy** — A value that PHP treats as true or false in a condition.
`0`, `0.0`, `''`, `'0'`, `null`, `false`, and empty arrays are falsy. Most other
values are truthy.

**Countable** — An array, or an object that implements PHP's `Countable`
interface. `count()` accepts countable values.

**Haystack / needle** — In `strpos($haystack, $needle)`, the haystack is the
string being searched and the needle is the text being looked for.

**Magic constant** — A PHP constant whose value depends on where it is used.
Examples: `__DIR__` is the current file's directory, and `__FILE__` is the
current file's path.

**Variable variable** — A variable whose name is stored in another variable.
For example, if `$name = 'token'`, then `$$name` is the same as `$token`.
`${$key}` is another spelling.

**Interpolated string** — A double-quoted string that contains variables, such
as `"Hello $name"`.

## Security terms

**Source** — Where untrusted data enters the program, such as `$_GET`, `$_POST`,
request headers, cookies, or a file upload.

**Sink** — A dangerous operation that should not receive untrusted data, such as
SQL, shell commands, file paths, HTML output, or `eval()`.

**Sanitizer** — Code that makes data safe for a particular sink, such as
parameterized SQL, HTML escaping, or an allowlist.

**Object injection** — An attack where untrusted serialized data causes PHP to
create objects of attacker-chosen classes. Dangerous magic methods can then run
unexpected code.

**Magic method** — A PHP method with a special name that runs automatically,
such as `__wakeup()`, `__destruct()`, `__toString()`, or `__call()`.

**Nonce** — A value used once, often to prevent replay attacks. It must be
unpredictable when used for security.

**Salt** — Random data added to a password before hashing so identical passwords
do not produce identical hashes.

**Unsalted hash** — A hash created without a salt. It is cheaper to crack than a
salted password hash.

**Secure cookie flag** — Tells the browser to send the cookie only over HTTPS.

**HttpOnly cookie flag** — Tells the browser to hide the cookie from JavaScript.

**SameSite cookie attribute** — Controls whether the browser sends a cookie with
cross-site requests. Common values are `Lax` and `Strict`.

**JSON round-trip** — Encoding data to JSON and immediately decoding it again,
such as `json_decode(json_encode($value), true)`.

**eval** — A PHP function that runs a string as code. It is dangerous when any
part of the string is controlled by a user.

**String assertion** — `assert('...')` with a string argument. Older PHP
versions evaluated the string as code.
