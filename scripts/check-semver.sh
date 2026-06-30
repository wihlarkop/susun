#!/usr/bin/env bash
set -euo pipefail

baseline="${SUSUN_SEMVER_BASELINE:-origin/main}"
baseline_dir="${TMPDIR:-/tmp}/susun-semver-baseline-$$"

cleanup() {
  git worktree remove --force "$baseline_dir" >/dev/null 2>&1 || rm -rf "$baseline_dir"
}
trap cleanup EXIT

git worktree add --detach "$baseline_dir" "$baseline" >/dev/null

current_packages="$(cargo metadata --no-deps --format-version 1 | python -c '
import json
import sys

metadata = json.load(sys.stdin)
for package in metadata["packages"]:
    if package.get("source") is None:
        print(package["name"])
')"

baseline_packages="$(cargo metadata --manifest-path "$baseline_dir/Cargo.toml" --no-deps --format-version 1 | python -c '
import json
import sys

metadata = json.load(sys.stdin)
for package in metadata["packages"]:
    if package.get("source") is None:
        print(package["name"])
')"

common_packages="$(
  comm -12 \
    <(printf '%s\n' "$current_packages" | sort -u) \
    <(printf '%s\n' "$baseline_packages" | sort -u)
)"

if [ -z "$common_packages" ]; then
  printf '%s\n' "no common workspace packages found for semver comparison" >&2
  exit 1
fi

while IFS= read -r package; do
  [ -n "$package" ] || continue
  printf 'checking semver for %s against %s\n' "$package" "$baseline"
  cargo semver-checks check-release --package "$package" --baseline-rev "$baseline"
done <<< "$common_packages"
