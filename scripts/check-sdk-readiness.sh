#!/usr/bin/env bash
set -euo pipefail

cargo check -p susun -p susun-cli
cargo check -p susun --examples
cargo test -p susun --test analyzer
cargo test -p susun-cli --test cli
