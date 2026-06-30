#!/usr/bin/env bash
set -eu

failures=0

fail() {
  printf 'release policy violation: %s\n' "$1" >&2
  failures=$((failures + 1))
}

msrv="$(awk -F'"' '/rust-version[[:space:]]*=/ { print $2; exit }' Cargo.toml)"
version="$(awk -F'"' '/^version[[:space:]]*=/ { print $2; exit }' Cargo.toml)"

[ -n "$msrv" ] || fail "workspace rust-version is missing"
[ -n "$version" ] || fail "workspace version is missing"

if [ "$msrv" != "1.85" ]; then
  fail "workspace rust-version must remain 1.85 unless the MSRV policy is intentionally updated"
fi

grep -q "dtolnay/rust-toolchain@$msrv" .github/workflows/ci.yml \
  || fail "CI MSRV toolchain must match workspace rust-version $msrv"

for manifest in crates/*/Cargo.toml; do
  grep -q 'version.workspace[[:space:]]*=[[:space:]]*true' "$manifest" \
    || fail "$manifest must inherit version.workspace"
  grep -q 'rust-version.workspace[[:space:]]*=[[:space:]]*true' "$manifest" \
    || fail "$manifest must inherit rust-version.workspace"
done

grep -Eq '^## Unreleased[[:space:]]*$' CHANGELOG.md \
  || fail "CHANGELOG.md must contain an Unreleased section"
grep -q "$version" CHANGELOG.md \
  || fail "CHANGELOG.md must mention workspace version $version before release"

grep -q 'cargo-semver-checks --version 0\.48\.0 --locked' .github/workflows/ci.yml \
  || fail "CI must install pinned cargo-semver-checks 0.48.0"
grep -q 'bash scripts/check-semver\.sh' .github/workflows/ci.yml \
  || fail "CI must run scripts/check-semver.sh"
grep -q 'cargo semver-checks check-release --package "$package" --baseline-rev "$baseline"' scripts/check-semver.sh \
  || fail "scripts/check-semver.sh must run package-scoped semver checks against the baseline"

if [ "$failures" -ne 0 ]; then
  exit 1
fi

printf 'release policy checks passed\n'
