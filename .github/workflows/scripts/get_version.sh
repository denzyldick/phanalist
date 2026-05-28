#!/usr/bin/env bash
set -euo pipefail
grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)".*/\1/'
