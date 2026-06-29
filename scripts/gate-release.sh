#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase5.sh

cargo audit

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check
else
  printf '%s\n' "cargo-deny is required for a full release gate" >&2
  printf '%s\n' "install with: cargo install cargo-deny --version 0.18.3 --locked" >&2
  exit 1
fi
