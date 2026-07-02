#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase12.sh
bash scripts/check-release-policy.sh
bash scripts/generate-release-docs.sh
git diff --exit-code -- docs/generated/capability-and-compatibility.md

# TODO(cargo-audit): temporarily disabled. rustsec/cvss cannot parse
# RUSTSEC-2026-0124's CVSS:4.0 vector ("unsupported CVSS version: 4.0"),
# so cargo-audit fails to load the advisory database entirely regardless
# of install method or version (confirmed on cargo-audit 0.22.1 and
# 0.22.2, both via `cargo install` and the taiki-e/install-action
# prebuilt binary). Re-enable once upstream ships a fix.
# cargo audit

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
