#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase2.sh

cargo run -p susun-cli -- up --help >/dev/null
cargo run -p susun-cli -- down --help >/dev/null
cargo run -p susun-cli -- ps --help >/dev/null
cargo run -p susun-cli -- logs --help >/dev/null

if [ "${SUSUN_RUN_DOCKER_GATE:-0}" = "1" ]; then
  bash scripts/run-docker-integration.sh
else
  printf '%s\n' "skipped Docker integration gate: set SUSUN_RUN_DOCKER_GATE=1"
fi
