#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase4.sh

cargo run -p susun-cli -- build --help >/dev/null
cargo run -p susun-cli -- run --help >/dev/null
cargo run -p susun-cli -- exec --help >/dev/null
cargo run -p susun-cli -- events --help >/dev/null
cargo run -p susun-cli -- wait --help >/dev/null
cargo run -p susun-cli -- cp --help >/dev/null
cargo run -p susun-cli -- port --help >/dev/null
cargo run -p susun-cli -- compatibility >/dev/null
cargo run -p susun-cli -- compatibility --corpus fixtures/compatibility/corpus.json >/dev/null
cargo run -p susun-cli -- compatibility --security-audit fixtures/compatibility/corpus.json >/dev/null
cargo bench --workspace --no-run

if [ "${SUSUN_RUN_COMPAT_GATE:-0}" = "1" ]; then
  bash scripts/run-compatibility-matrix.sh
else
  printf '%s\n' "skipped compatibility artifact gate: set SUSUN_RUN_COMPAT_GATE=1"
fi
