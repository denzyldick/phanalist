#!/usr/bin/env bash
set -euo pipefail
NEW_VERSION="${1?Usage: $0 <new_version>}"
if [[ "$(uname -s)" == "Darwin" ]]; then
  sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
else
  sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
fi
