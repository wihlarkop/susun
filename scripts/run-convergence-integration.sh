#!/usr/bin/env bash
set -euo pipefail

cargo test -p susun-convergence
cargo test -p susun-runtime

help_output="$(cargo run -p susun-cli -- up --help)"
grep -q -- "--scale" <<<"$help_output"
grep -q -- "--remove-orphans" <<<"$help_output"
grep -q -- "--force-recreate" <<<"$help_output"
grep -q -- "--no-recreate" <<<"$help_output"
grep -q -- "--renew-anon-volumes" <<<"$help_output"

set +e
conflict_output="$(cargo run -p susun-cli -- up --force-recreate --no-recreate 2>&1)"
conflict_status=$?
set -e
if [[ "$conflict_status" -eq 0 ]]; then
  echo "expected conflicting recreate flags to fail" >&2
  exit 1
fi
grep -q -- "conflicts" <<<"$conflict_output"

SUSUN_DOCKER_REQUIRED=1 cargo test -p susun-engine-bollard --test adapter_contract -- --nocapture
