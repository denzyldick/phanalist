#!/usr/bin/env bash
set -euo pipefail
cargo publish --dry-run --token "${CRATES_IO_TOKEN}"
