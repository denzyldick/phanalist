#!/usr/bin/env bash
# Framework-level e2e: route -> controller -> service.
#
# Builds a throwaway Symfony-shaped Composer/PSR-4 project, checks that
# phanalist records the inferred call chain in the generated test docblock and
# in JSON/SARIF/Code Climate reports, then verifies both the generated test and
# a route-level reproducer go red on buggy code and green after only the service
# source is fixed.
#
# Usage:
#   scripts/verify-framework-reproducer.sh
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

project=$(mktemp -d "${TMPDIR:-/tmp}/phanalist-framework-XXXXXX")
mkdir -p "$project/src/Service" "$project/src/Controller" "$project/src/Http" "$project/src/Routing/Attribute"
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
  "name": "demo/framework-shop",
  "autoload": {
    "psr-4": {
      "App\\": "src/"
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
        <testsuite name="Framework Reproducer">
            <directory>tests</directory>
        </testsuite>
    </testsuites>
</phpunit>
XML

cat >"$project/src/Service/DiscountCalculator.php" <<'PHP'
<?php

namespace App\Service;

final class DiscountCalculator
{
    public function discountedTotal(int $total, int $divisor): float
    {
        return $total / $divisor;
    }
}
PHP

cat >"$project/src/Routing/Attribute/Route.php" <<'PHP'
<?php

namespace App\Routing\Attribute;

#[\Attribute(\Attribute::TARGET_METHOD)]
final class Route
{
    /**
     * @param list<string> $methods
     */
    public function __construct(
        public string $path,
        public array $methods = ['GET'],
    ) {
    }
}
PHP

cat >"$project/src/Http/Request.php" <<'PHP'
<?php

namespace App\Http;

final class Request
{
    /**
     * @param array<string, string> $post
     */
    public function __construct(private array $post)
    {
    }

    public function input(string $key, string $default = ''): string
    {
        return $this->post[$key] ?? $default;
    }
}
PHP

cat >"$project/src/Http/Response.php" <<'PHP'
<?php

namespace App\Http;

final class Response
{
    public function __construct(private string $body)
    {
    }

    public function body(): string
    {
        return $this->body;
    }
}
PHP

cat >"$project/src/Controller/CheckoutController.php" <<'PHP'
<?php

namespace App\Controller;

use App\Http\Request;
use App\Http\Response;
use App\Routing\Attribute\Route;
use App\Service\DiscountCalculator;

final class CheckoutController
{
    public function __construct(private DiscountCalculator $calculator)
    {
    }

    #[Route('/checkout', methods: ['POST'])]
    public function submit(Request $request): Response
    {
        $total = (int) $request->input('total', '100');
        $divisor = (int) $request->input('divisor', '0');

        return new Response('total=' . $this->calculator->discountedTotal($total, $divisor));
    }
}
PHP

cat >"$project/src/Routing/Router.php" <<'PHP'
<?php

namespace App\Routing;

use App\Controller\CheckoutController;
use App\Http\Request;
use App\Http\Response;
use App\Routing\Attribute\Route;
use App\Service\DiscountCalculator;
use ReflectionClass;
use ReflectionMethod;

final class Router
{
    public function dispatch(string $method, string $path, array $post): Response
    {
        $controller = new CheckoutController(new DiscountCalculator());
        $reflection = new ReflectionClass($controller);

        foreach ($reflection->getMethods(ReflectionMethod::IS_PUBLIC) as $action) {
            foreach ($action->getAttributes(Route::class) as $attribute) {
                /** @var Route $route */
                $route = $attribute->newInstance();
                if ($route->path !== $path || !in_array(strtoupper($method), $route->methods, true)) {
                    continue;
                }

                return $action->invoke($controller, new Request($post));
            }
        }

        return new Response('not found');
    }
}
PHP

cat >"$project/tests/FrameworkReproducerTest.php" <<'PHP'
<?php

declare(strict_types=1);

use App\Routing\Router;
use PHPUnit\Framework\TestCase;

require_once __DIR__ . '/../vendor/autoload.php';

final class FrameworkReproducerTest extends TestCase
{
    public function testPostCheckoutDoesNotCrash(): void
    {
        $router = new Router();

        try {
            $response = $router->dispatch('POST', '/checkout', [
                'total' => '100',
                'divisor' => '0',
            ]);
        } catch (\DivisionByZeroError $error) {
            $this->fail('POST /checkout crashed: ' . $error->getMessage());
        }

        $this->assertStringStartsWith('total=', $response->body());
    }
}
PHP

phpunit_command=${PHPUNIT_CMD[*]}
printf '#!/bin/sh\nexec %s "$@"\n' "$phpunit_command" >"$project/vendor/bin/phpunit"
chmod +x "$project/vendor/bin/phpunit"

echo "== Generate tests for the service finding =="
set +e
(
  cd "$project"
  "$BIN" -s src \
    --rules E0031 \
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
if [ "${#generated[@]}" -ne 1 ]; then
  echo "expected 1 generated test, got ${#generated[@]}" >&2
  exit 1
fi

generated_file=${generated[0]}
for needle in \
  'Call chain (inferred): POST /checkout' \
  'CheckoutController::submit() at src/Controller/CheckoutController.php:' \
  'DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:'; do
  if ! grep -qF "$needle" "$generated_file"; then
    echo "generated test is missing call-chain evidence: $needle" >&2
    sed -n '1,20p' "$generated_file" >&2
    exit 1
  fi
done
echo "  ok: generated docblock records POST /checkout -> controller -> service"
php -l "$generated_file" >/dev/null

echo "== Check report outputs =="
for format in json sarif codeclimate; do
  set +e
  (
    cd "$project"
    "$BIN" -s src --rules E0031 --output-format "$format" \
      >"report-$format.out" 2>"report-$format.err"
  )
  report_status=$?
  set -e
  if [ "$report_status" -ne 0 ] && [ "$report_status" -ne 70 ]; then
    echo "phanalist $format report failed with exit $report_status" >&2
    cat "$project/report-$format.err" >&2
    exit 1
  fi
done
python3 - "$project" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])

def load(name):
    return json.loads((root / name).read_text())

json_report = load("report-json.out")
violations = [
    violation
    for group in json_report["files"].values()
    for violation in group
]
assert len(violations) == 1, violations
assert violations[0]["call_chain"][0].startswith("POST /checkout"), violations[0]

sarif = load("report-sarif.out")
result = sarif["runs"][0]["results"][0]
assert "Call chain (inferred): POST /checkout" in result["message"]["text"], result["message"]
assert "Call chain (inferred): POST /checkout" in result["message"]["markdown"], result["message"]
assert result["properties"]["phanalist/callChain"][0].startswith("POST /checkout"), result["properties"]

code_climate = load("report-codeclimate.out")
issue = code_climate[0]
assert issue["call_chain"][0].startswith("POST /checkout"), issue
assert "Call chain (inferred): POST /checkout" in issue["content"]["body"], issue["content"]
PY
echo "  ok: JSON, SARIF and Code Climate reports include the chain"

(cd "$project" && composer dump-autoload --no-interaction >autoload.out 2>&1)
sha256sum "$generated_file" >"$project/generated.sha"

echo "== Red run: buggy code, generated test and reproducer must fail =="
set +e
(cd "$project" && composer run-script phanalist-tests --no-interaction) >"$project/red-generated.out" 2>&1
red_generated=$?
(cd "$project" && vendor/bin/phpunit tests/FrameworkReproducerTest.php) >"$project/red-route.out" 2>&1
red_route=$?
set -e
if [ "$red_generated" -eq 0 ] || [ "$red_route" -eq 0 ]; then
  echo "expected both red runs to fail (generated=$red_generated route=$red_route)" >&2
  tail -n 20 "$project/red-generated.out" >&2
  tail -n 20 "$project/red-route.out" >&2
  exit 1
fi
grep -q 'POST /checkout crashed: Division by zero' "$project/red-route.out"
echo "  ok: generated test and route reproducer are red"

python3 - "$project/src/Service/DiscountCalculator.php" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
old = "        return $total / $divisor;"
new = "        if ($divisor === 0) {\n            return 0.0;\n        }\n\n        return $total / $divisor;"
assert old in text
path.write_text(text.replace(old, new, 1))
PY

echo "== Green run: same tests, service source fixed =="
(cd "$project" && composer run-script phanalist-tests --no-interaction) >"$project/green-generated.out" 2>&1
(cd "$project" && vendor/bin/phpunit tests/FrameworkReproducerTest.php) >"$project/green-route.out" 2>&1
grep -q 'OK (1 test, 1 assertion)' "$project/green-generated.out"
grep -q 'OK (1 test, 1 assertion)' "$project/green-route.out"
(cd "$project" && sha256sum -c generated.sha >/dev/null)
echo "  ok: generated test and route reproducer are green with unchanged test file"
echo "ALL FRAMEWORK REPRODUCER CHECKS PASSED"
