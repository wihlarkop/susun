#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase3.sh

cargo run -p susun-cli -- inspect-plan --help >/dev/null
cargo run -p susun-cli -- -f fixtures/compatibility/override-runtime-convergence/compose.yaml plan up --format json >/dev/null

if [ "${SUSUN_RUN_CONVERGENCE_GATE:-0}" = "1" ]; then
  bash scripts/run-convergence-integration.sh
else
  printf '%s\n' "skipped convergence integration gate: set SUSUN_RUN_CONVERGENCE_GATE=1"
fi
