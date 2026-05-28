#!/usr/bin/env bash
set -euo pipefail
cargo publish --token "${CRATES_IO_TOKEN}"
