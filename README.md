# susun

Susun is a Rust SDK and CLI for loading, normalizing, validating, and inspecting
Docker Compose projects without talking to a Docker daemon.

Phase 1 focuses on Compose specification analysis:

- load one or more Compose files;
- resolve `.env`, `--env-file`, and process environment interpolation;
- merge repeated `-f` files with field-aware Compose rules;
- normalize supported short and long syntax into a canonical project model;
- validate active service references, ports, health dependencies, and cycles;
- emit deterministic human or JSON diagnostics.

The active design source is under `docs/superpowers/`.

## CLI

```powershell
cargo run -p susun-cli -- -f compose.yaml check
cargo run -p susun-cli -- -f compose.yaml config
cargo run -p susun-cli -- -f compose.yaml --format json check
```

Exit codes:

- `0`: success, no error diagnostics
- `1`: user/project diagnostics were found
- `2`: operational failure, such as an unreadable file

## Library

```rust
use susun::Analyzer;

let result = Analyzer::new("compose.yaml").analyze()?;
if result.report.has_errors() {
    eprintln!("project has diagnostics");
}
```

## Phase 1 Gate

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

`cargo test --workspace` is expected to be restored as part of the verification
track; feature implementation is currently prioritized.
