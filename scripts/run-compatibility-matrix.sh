#!/usr/bin/env bash
set -eu

out_dir="${SUSUN_COMPAT_OUT_DIR:-target/compatibility}"
mkdir -p "$out_dir"

cargo run -p susun-cli -- compatibility > "$out_dir/capability-matrix.json"
cargo run -p susun-cli -- compatibility --corpus fixtures/compatibility/corpus.json > "$out_dir/oracle-plan.json"
cargo run -p susun-cli -- compatibility --security-audit fixtures/compatibility/corpus.json > "$out_dir/security-audit.json"

cp fixtures/compatibility/version-matrix.json "$out_dir/version-matrix.json"
cp fixtures/compatibility/performance-budgets.json "$out_dir/performance-budgets.json"

if [ "${SUSUN_RUN_COMPOSE_ORACLE:-0}" = "1" ]; then
  docker compose version > "$out_dir/docker-compose-version.txt"
else
  printf '%s\n' "skipped: set SUSUN_RUN_COMPOSE_ORACLE=1 to execute Docker Compose oracle checks" > "$out_dir/docker-compose-version.txt"
fi

printf '%s\n' "wrote compatibility artifacts to $out_dir"
