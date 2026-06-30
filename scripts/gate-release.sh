#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase5.sh
bash scripts/check-release-policy.sh
bash scripts/generate-release-docs.sh
git diff --exit-code -- docs/generated/capability-and-compatibility.md

cargo audit

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check
else
  printf '%s\n' "cargo-deny is required for a full release gate" >&2
  printf '%s\n' "install with: cargo install cargo-deny --version 0.18.3 --locked" >&2
  exit 1
fi

if command -v cargo-semver-checks >/dev/null 2>&1; then
  bash scripts/check-semver.sh
else
  printf '%s\n' "cargo-semver-checks is required for a full release gate" >&2
  printf '%s\n' "install with: cargo install cargo-semver-checks --version 0.48.0 --locked" >&2
  exit 1
fi
