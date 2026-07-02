#!/usr/bin/env bash
set -euo pipefail

version="$(
python - <<'PY'
import tomllib
from pathlib import Path

manifest = tomllib.loads(Path("Cargo.toml").read_text())
print(manifest["workspace"]["package"]["version"])
PY
)"

tag="${GITHUB_REF_NAME:-}"
if [ -n "$tag" ] && [ "$tag" != "v$version" ]; then
  printf 'release tag %s does not match workspace version v%s\n' "$tag" "$version" >&2
  exit 1
fi

# Topological order over both [dependencies] and [dev-dependencies]:
# `cargo publish --locked` requires every workspace dependency
# (including dev-dependencies that carry a version) to already be
# resolvable on crates.io, so a crate must not appear before anything
# it depends on, directly or via dev-dependencies.
packages=(
  susun-model
  susun-secret
  susun-source
  susun-build
  susun-diagnostics
  susun-engine
  susun-normalize
  susun-graph
  susun-loader
  susun-validation
  susun-planner
  susun-testkit
  susun-watch
  susun-engine-bollard
  susun-runtime
  susun-convergence
  susun
  susun-compat
  susun-cli
)

if [ "${SUSUN_PUBLISH_DRY_RUN:-}" = "1" ]; then
  printf 'publish order for v%s:\n' "$version"
  printf '%s\n' "${packages[@]}"
  exit 0
fi

if [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then
  printf '%s\n' "CARGO_REGISTRY_TOKEN is required to publish crates" >&2
  exit 1
fi

already_published() {
  local package="$1"
  local ver="$2"
  local status

  status="$(curl -s -o /dev/null -w '%{http_code}' "https://crates.io/api/v1/crates/${package}/${ver}")"
  [ "$status" = "200" ]
}

publish_package() {
  local package="$1"
  local attempt

  for attempt in 1 2 3 4 5; do
    if cargo publish -p "$package" --locked --token "$CARGO_REGISTRY_TOKEN"; then
      return 0
    fi

    if [ "$attempt" -eq 5 ]; then
      printf 'publishing %s failed after %s attempts\n' "$package" "$attempt" >&2
      return 1
    fi

    printf 'publish attempt %s for %s failed; waiting for registry/index propagation\n' "$attempt" "$package" >&2
    sleep 30
  done
}

for package in "${packages[@]}"; do
  # Idempotent: a prior run may have published some prefix of this list
  # before failing partway through (crates.io has no delete, only yank),
  # so skip anything already present at this exact version.
  if already_published "$package" "$version"; then
    printf 'skipping %s %s (already published)\n' "$package" "$version"
    continue
  fi

  printf 'publishing %s %s\n' "$package" "$version"
  publish_package "$package"
done
