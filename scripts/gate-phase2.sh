#!/usr/bin/env bash
set -euo pipefail

bash scripts/check-architecture.sh
bash scripts/check-diagnostics.sh
bash scripts/check-schemas.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

cargo run -p susun-cli -- --help >/dev/null
cargo run -p susun-cli -- -f fixtures/compatibility/analysis-minimal/compose.yaml config --format json >/dev/null
cargo run -p susun-cli -- -f fixtures/compatibility/analysis-minimal/compose.yaml plan up --format json >/dev/null
