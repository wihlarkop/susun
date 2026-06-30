#!/usr/bin/env bash
set -euo pipefail

cargo fuzz run dockerignore_parse -- -runs="${SUSUN_FUZZ_RUNS:-256}"
cargo fuzz run plan_deserialize -- -runs="${SUSUN_FUZZ_RUNS:-256}"
