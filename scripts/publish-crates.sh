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

packages=(
  susun-source
  susun-secret
  susun-model
  susun-diagnostics
  susun-normalize
  susun-engine
  susun-build
  susun-graph
  susun-validation
  susun-loader
  susun-planner
  susun-convergence
  susun-testkit
  susun-runtime
  susun-watch
  susun-engine-bollard
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
  printf 'publishing %s %s\n' "$package" "$version"
  publish_package "$package"
done
