#!/usr/bin/env bash
# End-to-end verification for generated PHPUnit regression tests.
#
# Creates a throwaway Composer/PSR-4 project, enables every test-generating
# verification rule, runs the generated tests red on buggy code, fixes only the
# source, then reruns the exact same tests green.
#
# Usage:
#   scripts/verify-generated-tests.sh
#
# Environment:
#   PHANALIST_BIN   path to the phanalist binary (default: target/debug/phanalist)
#   PHPUNIT_PHAR    path to a phpunit phar used when phpunit is not on PATH
#   KEEP_E2E=1      keep the generated project and print its path

set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BIN=${PHANALIST_BIN:-"$ROOT_DIR/target/debug/phanalist"}
PHPUNIT_PHAR=${PHPUNIT_PHAR:-/tmp/opencode/phpunit/phpunit.phar}

if [ ! -x "$BIN" ]; then
  echo "phanalist binary not found at $BIN" >&2
  echo "Run: cargo build --bin phanalist" >&2
  exit 1
fi

if [ -n "${PHPUNIT:-}" ]; then
  read -r -a PHPUNIT_CMD <<<"$PHPUNIT"
elif command -v phpunit >/dev/null 2>&1; then
  PHPUNIT_CMD=("$(command -v phpunit)")
elif [ -f "$PHPUNIT_PHAR" ]; then
  PHPUNIT_CMD=(php "$PHPUNIT_PHAR")
else
  echo "phpunit not found. Install it or set PHPUNIT/PHPUNIT_PHAR." >&2
  exit 1
fi

if ! command -v composer >/dev/null 2>&1; then
  echo "composer is required for this scenario." >&2
  exit 1
fi

project=$(mktemp -d "${TMPDIR:-/tmp}/phanalist-e2e-XXXXXX")
mkdir -p "$project/src/Domain" "$project/src/Collections" "$project/src/Text"
mkdir -p "$project/src/Customer" "$project/src/Billing"
mkdir -p "$project/tests/phanalist-generated" "$project/vendor/bin"

cleanup() {
  if [ "${KEEP_E2E:-0}" = "1" ]; then
    echo "kept project: $project"
  else
    rm -rf "$project"
  fi
}
trap cleanup EXIT

cat >"$project/composer.json" <<'JSON'
{
  "name": "demo/shop",
  "autoload": {
    "psr-4": {
      "Demo\\Shop\\": "src/"
    }
  }
}
JSON

cat >"$project/phpunit.xml.dist" <<'XML'
<?xml version="1.0"?>
<phpunit colors="false">
    <testsuites>
        <testsuite name="Phanalist Generated">
            <directory>tests/phanalist-generated</directory>
        </testsuite>
    </testsuites>
</phpunit>
XML

cat >"$project/src/Domain/PriceCalculator.php" <<'PHP'
<?php

namespace Demo\Shop\Domain;

final class PriceCalculator
{
    public static function discountedTotal(int $total, int $divisor): float
    {
        return $total / $divisor;
    }
}
PHP

cat >"$project/src/Domain/DiscountRules.php" <<'PHP'
<?php

namespace Demo\Shop\Domain;

function applyModulo(int $divisor): int
{
    return 10 % $divisor;
}

function applyIntdiv(int $divisor): int
{
    return intdiv(10, $divisor);
}

function splitEvenly(int $dividend, int $divisor): int
{
    return intdiv($dividend, $divisor);
}
PHP

cat >"$project/src/Collections/Catalog.php" <<'PHP'
<?php

namespace Demo\Shop\Collections;

function readConfig(int $key): mixed
{
    $config = ['currency' => 'EUR'];

    return $config[$key];
}

/**
 * @param list<int> $items
 */
function readItem(array $items, int $index): int
{
    return $items[$index];
}
PHP

cat >"$project/src/Text/Slug.php" <<'PHP'
<?php

namespace Demo\Shop\Text;

function charAt(int $index): string
{
    $value = 'abc';

    return $value[$index];
}
PHP

cat >"$project/src/Customer/Contact.php" <<'PHP'
<?php

namespace Demo\Shop\Customer;

class Messenger
{
    public function send(): void
    {
    }
}

function notify(?Messenger $messenger): void
{
    $messenger->send();
}
PHP

cat >"$project/src/Customer/Profile.php" <<'PHP'
<?php

namespace Demo\Shop\Customer;

class Settings
{
    public int $limit = 1;
}

function readLimit(?Settings $settings): int
{
    return $settings->limit;
}
PHP

cat >"$project/src/Billing/InvoiceCalculator.php" <<'PHP'
<?php

namespace Demo\Shop\Billing;

final class InvoiceCalculator
{
    public function __construct(private string $currency)
    {
    }

    public function split(int $divisor): float
    {
        return 100 / $divisor;
    }
}
PHP

phpunit_command=${PHPUNIT_CMD[*]}
printf '#!/bin/sh\nexec %s "$@"\n' "$phpunit_command" >"$project/vendor/bin/phpunit"
chmod +x "$project/vendor/bin/phpunit"

echo "== Configure project with phanalist init =="
(
  cd "$project"
  "$BIN" init --yes --src src --generate-tests tests/phanalist-generated --ci >init.out 2>&1
  composer dump-autoload --no-interaction >autoload.out 2>&1
)

echo "== Generate tests for E0031-E0039 =="
set +e
(
  cd "$project"
  "$BIN" --src src \
    --rules E0031 --rules E0032 --rules E0033 --rules E0034 --rules E0035 \
    --rules E0036 --rules E0037 --rules E0038 --rules E0039 \
    --generate-tests tests/phanalist-generated \
    --output-format json >scan.out 2>scan.err
)
scan_status=$?
set -e
if [ "$scan_status" -ne 0 ] && [ "$scan_status" -ne 70 ]; then
  echo "phanalist scan failed with exit $scan_status" >&2
  cat "$project/scan.err" >&2
  exit 1
fi

mapfile -d '' generated < <(find "$project/tests/phanalist-generated" -maxdepth 1 -name '*Test.php' -type f -print0 | sort -z)
echo "generated ${#generated[@]} test files"

if [ "${#generated[@]}" -ne 11 ]; then
  echo "expected 11 generated tests, got ${#generated[@]}" >&2
  printf '%s\n' "${generated[@]}" >&2
  cat "$project/scan.err" >&2
  exit 1
fi

for rule in E0031 E0032 E0033 E0034 E0035 E0036 E0037 E0038 E0039; do
  if ! grep -Rqs "Rule: $rule —" "$project/tests/phanalist-generated"; then
    echo "no generated test for $rule" >&2
    exit 1
  fi
done

grep -Rqs '\\Demo\\Shop\\Domain\\PriceCalculator::discountedTotal' "$project/tests/phanalist-generated"

for file in "${generated[@]}"; do
  php -l "$file" >/dev/null
done

find "$project/tests/phanalist-generated" -maxdepth 1 -name '*Test.php' -type f -print0 \
  | sort -z \
  | xargs -0 sha256sum >"$project/generated.sha"

echo "== Red run: buggy source, generated tests must fail =="
set +e
(cd "$project" && composer run-script phanalist-tests --no-interaction) >"$project/red.out" 2>&1
red_status=$?
set -e
if [ "$red_status" -eq 0 ]; then
  echo "PHPUnit passed on buggy code" >&2
  tail -n 30 "$project/red.out" >&2
  exit 1
fi
if ! grep -Eq 'Errors: 11|Failures: 11' "$project/red.out"; then
  echo "expected 11 PHPUnit errors/failures in red run" >&2
  tail -n 30 "$project/red.out" >&2
  exit 1
fi
echo "  red ok: $(grep -E 'ERRORS!|FAILURES!' "$project/red.out" | tail -n 1)"

python3 - "$project/src" <<'PY'
from pathlib import Path
import sys

root = Path(sys.argv[1])
replacements = {
    "Domain/PriceCalculator.php": [
        ("        return $total / $divisor;", "        if ($divisor === 0) {\n            return 0.0;\n        }\n\n        return $total / $divisor;"),
    ],
    "Domain/DiscountRules.php": [
        ("    return 10 % $divisor;", "    if ($divisor === 0) {\n        return 0;\n    }\n\n    return 10 % $divisor;"),
        ("    return intdiv(10, $divisor);", "    if ($divisor === 0) {\n        return 0;\n    }\n\n    return intdiv(10, $divisor);"),
        ("    return intdiv($dividend, $divisor);", "    if ($divisor === 0) {\n        return 0;\n    }\n\n    if ($dividend === PHP_INT_MIN && $divisor === -1) {\n        return 0;\n    }\n\n    return intdiv($dividend, $divisor);"),
    ],
    "Collections/Catalog.php": [
        ("    return $config[$key];", "    return $config[$key] ?? null;"),
        ("    return $items[$index];", "    return $items[$index] ?? 0;"),
    ],
    "Text/Slug.php": [
        ("    return $value[$index];", "    if ($index < 0 || $index >= strlen($value)) {\n        return '';\n    }\n\n    return $value[$index];"),
    ],
    "Customer/Contact.php": [
        ("    $messenger->send();", "    if ($messenger === null) {\n        return;\n    }\n\n    $messenger->send();"),
    ],
    "Customer/Profile.php": [
        ("    return $settings->limit;", "    if ($settings === null) {\n        return 0;\n    }\n\n    return $settings->limit;"),
    ],
    "Billing/InvoiceCalculator.php": [
        ("        return 100 / $divisor;", "        if ($divisor === 0) {\n            return 0.0;\n        }\n\n        return 100 / $divisor;"),
    ],
}
for relative, edits in replacements.items():
    path = root / relative
    text = path.read_text()
    for old, new in edits:
        assert old in text, (relative, old)
        text = text.replace(old, new, 1)
    path.write_text(text)
PY

echo "== Green run: same tests, fixed sources =="
(cd "$project" && composer run-script phanalist-tests --no-interaction) >"$project/green.out" 2>&1
if ! grep -q 'OK (11 tests, 11 assertions)' "$project/green.out"; then
  echo "expected OK (11 tests, 11 assertions) after fixes" >&2
  tail -n 30 "$project/green.out" >&2
  exit 1
fi
echo "  green ok: $(grep -E 'OK \(' "$project/green.out" | tail -n 1)"

(cd "$project/tests/phanalist-generated" && sha256sum -c "$project/generated.sha" >/dev/null)
echo "ALL MULTI-RULE REAL SCENARIO CHECKS PASSED"
