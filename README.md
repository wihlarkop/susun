# susun

Susun is a Rust SDK and CLI for loading, normalizing, validating, planning, and
running Docker Compose projects through structured Rust APIs.

The current unpublished `0.2.0` line covers the documented v0.2 supported
subset: Compose analysis, daemon-free planning, Docker runtime execution,
convergence decisions, BuildKit-oriented build inputs, compatibility evidence,
and release-readiness gates.

## SDK

Use `SusunWorkspace` for application and desktop-tool integrations. It mirrors
the Compose context flags a CLI would accept, but returns structured data.

```rust
use susun::SusunWorkspace;

let project = SusunWorkspace::from_file("compose.yaml")
    .with_profile("debug")
    .analyze()?;

let summary = project.summary();
println!(
    "{} service(s), {} active",
    summary.service_count,
    summary.active_service_count
);

let outcome = project.dry_run_up(false)?;
if let Some(plan) = outcome.plan {
    println!("plan {} has {} action(s)", plan.plan_id, plan.actions.len());
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Lower-level facades remain available when callers need explicit control:

- `Analyzer` for source-aware Compose analysis.
- `Planner` for explicit capability/snapshot planning.
- `up_with_engine` and `down_with_engine` for runtime execution with a supplied
  engine adapter.

## CLI

```powershell
cargo run -p susun-cli -- -f compose.yaml check
cargo run -p susun-cli -- -f compose.yaml config
cargo run -p susun-cli -- -f compose.yaml summary
cargo run -p susun-cli -- -f compose.yaml --format json summary
cargo run -p susun-cli -- -f compose.yaml plan up
```

Exit codes:

- `0`: success, no blocking diagnostics
- `1`: user/project diagnostics or blocked planning
- `2`: operational failure, such as unreadable files or engine errors

## Compatibility

The generated compatibility report is tracked in
`docs/generated/capability-and-compatibility.md`. It records the current
capability matrix, compatibility corpus, security audit, version matrix,
performance budgets, real-world compatibility catalog, and release-readiness
status.

Susun intentionally does not claim full Docker Compose bug-for-bug parity yet.
Unsupported and deferred behavior is reported through diagnostics, capability
metadata, or release-readiness deferred-work sections.

## Verification

Common local checks:

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test -p susun --test analyzer
cargo test -p susun-cli --test cli
```

The full release gate is wired through GitHub Actions and the shell scripts in
`scripts/`.
