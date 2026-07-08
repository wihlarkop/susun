# susun

Susun is a Rust SDK and CLI for loading, normalizing, validating, planning, and
running Docker Compose projects through structured Rust APIs.

The current unpublished `0.1.0` line is the first public release candidate. It
covers Compose analysis, daemon-free planning, Docker runtime execution,
convergence decisions, BuildKit-oriented build inputs, SDK/CLI consumer
contracts, compatibility evidence, package-readiness checks, and final
release-candidate gates.

## SDK

Use `SusunWorkspace` for application and desktop-tool integrations. It mirrors
the Compose context flags a CLI would accept, but returns structured data.
Configured files, env file, deterministic env map, project name override, and
profiles are available through read-only accessors for application state UIs.

```rust
use susun::SusunWorkspace;

let project = SusunWorkspace::from_file("compose.yaml")
    .with_env_var("COMPOSE_PROJECT_NAME", "my-app")
    .with_profile("debug")
    .analyze()?;

if project.has_errors() {
    eprint!("{}", project.render_diagnostics());
    let diagnostics_json = project.render_diagnostics_summary_json()?;
    println!("{diagnostics_json}");
}

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

let down = project.dry_run_down_plan()?;
println!("down plan available: {}", down.is_some());
# Ok::<(), Box<dyn std::error::Error>>(())
```

When an application needs a plan based on the current runtime state but should
not execute yet, use `plan_up_with_engine` or `plan_down_with_engine` with a
supplied engine adapter. Plans and execution reports can be persisted through
the facade helpers `render_execution_plan_json`,
`parse_execution_plan_json`, `render_execution_report_json`, and
`parse_execution_report_json`; facade parsers reject unsupported plan schemas
and inconsistent report summaries. The full `RuntimeOperationResult` returned by
mutating SDK calls is versioned, uses redacted per-action error messages in
execution reports, and can also be persisted with
`render_runtime_operation_result_json` and
`parse_runtime_operation_result_json`, or summarized with
`RuntimeOperationSummary` and its JSON helpers. Failed mutating SDK calls can be
converted into redacted `RuntimeOperationErrorSummary` payloads for UI/API error
responses. Approval UIs can persist compact planning results through
`PlanOutcomeSummary`,
`render_plan_outcome_summary_json`, and `parse_plan_outcome_summary_json`.
Official SDK parse helpers validate schema versions and internal summary
consistency so persisted UI/API payloads fail fast when counts or statuses drift.
Analysis diagnostics are available as typed, versioned `DiagnosticReportSummary`
payloads through `diagnostics_summary`,
`render_diagnostic_report_summary_json`, and
`parse_diagnostic_report_summary_json`. System-level analysis failures can be
converted to display-safe `AnalysisErrorSummary` payloads without exposing local
Compose file paths.

For mutating runtime flows, analyze once and execute through the same
`SdkProject`:

```rust
# use std::sync::Arc;
# use susun::{SusunWorkspace, UpPlanOptions};
# async fn run(engine: Arc<impl susun::ContainerEngine + 'static>) -> Result<(), Box<dyn std::error::Error>> {
let project = SusunWorkspace::from_file("compose.yaml").analyze()?;
let result = project.up_with_engine(engine, UpPlanOptions::default()).await?;
println!(
    "executed {} action(s)",
    result.report.summary.total_actions
);
# Ok(())
# }
```

Lower-level facades remain available when callers need explicit control:

- `Analyzer` for source-aware Compose analysis.
- `Planner` for explicit capability/snapshot planning.
- `up_with_engine` and `down_with_engine` for runtime execution with a supplied
  engine adapter.

Advanced integrations can call `SdkProject::into_analysis` or
`SdkProject::into_parts` when lower-level crates need owned analysis data.
For read-only inspection, `SdkProject::selection`, `graph`, and `source_map`
avoid reaching through `analysis()` directly.

The facade crate also re-exports common SDK types such as `Project`,
`ProjectName`, `EngineCapabilities`, `EngineSnapshot`, `ProjectIdentity`,
`UpPlanOptions`, `DownPlanOptions`, `ExecutionPlan`, `PlanOutcome`, and
`ExecutionReport`, so most applications can depend on `susun` first and reach
for lower-level crates only when they need specialized extension points.

For desktop tools and daemons that manage Docker-compatible runtimes, Susun also
provides neutral connection profile and runtime doctor DTOs. The public facade
exposes `EngineConnectionProfile`, `EngineConnectionProfileId`,
`EngineConnectionDisplayName`, `RuntimeDoctorReport`, and
`RuntimeDoctorStatus`; concrete probing stays in adapter crates such as
`susun-engine-bollard`.
`EngineConnectionProfileSet` validates duplicate profile IDs and default
selection, so applications such as Susun Studio can own storage while reusing
Susun's neutral profile semantics. When profile JSON is persisted or accepted
through an API, deserialize it through these Susun types so constructor-backed
validation runs before use. The `susun` facade provides
`parse_engine_connection_profile_set_json` and
`render_engine_connection_profile_set_json` for that boundary. Profile JSON is
configuration data, not a redacted UI summary: endpoint fields can contain local
socket paths, named pipes, remote hosts, and TLS certificate/key paths, so store
it only in protected application storage. For UI lists, logs, and daemon API
responses, convert protected profiles into `EngineConnectionProfileSetSummary`
and render them with `render_engine_connection_profile_set_summary_json`; the
summary contains only profile ids, display names, endpoint kind, default
selection, and Susun's redacted endpoint token.

For project dashboards, `runtime_status_from_snapshot` converts a neutral
`EngineSnapshot` plus `ProjectIdentity` into a compact `RuntimeStatusSummary`
with service/container counts and JSON helpers. This is the recommended DTO for
desktop UI status panels; raw snapshots remain available for advanced tooling.
`runtime_overview` combines a `RuntimeDoctorReport` with optional project status
into a single `RuntimeOverview` payload for dashboard health cards and daemon
API responses. Versioned SDK summary parse helpers reject unsupported schema
versions, so consumers can persist them or exchange them through local APIs
with a stable upgrade boundary. SDK consumers can call
`SdkProject::runtime_status_from_snapshot`,
`SdkProject::runtime_status_with_engine`, or
`SdkProject::runtime_overview_with_engine` when they want the facade to reuse
the analyzed project identity directly.

```powershell
cargo run -p susun --example runtime_doctor
cargo run -p susun --example local_docker_up_down -- compose.yaml
```

## CLI

```powershell
cargo run -p susun-cli -- -f compose.yaml check
cargo run -p susun-cli -- -f compose.yaml config
cargo run -p susun-cli -- -f compose.yaml summary
cargo run -p susun-cli -- -f compose.yaml --format json summary
cargo run -p susun-cli -- doctor
cargo run -p susun-cli -- --format json doctor
cargo run -p susun-cli -- -f compose.yaml overview
cargo run -p susun-cli -- -f compose.yaml --format json overview
cargo run -p susun-cli -- -f compose.yaml plan up
cargo run -p susun-cli -- -f compose.yaml status
cargo run -p susun-cli -- -f compose.yaml --format json status
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
powershell -ExecutionPolicy Bypass -File scripts\gate-phase12.ps1
```

The full release gate is wired through GitHub Actions and the shell scripts in
`scripts/`.

## Release

Crates are published from a tag through `.github/workflows/release-crates.yml`.
Before pushing `v0.1.0`, configure the repository Actions secret
`CARGO_REGISTRY_TOKEN`.

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The workflow runs `scripts/gate-release.sh` and then publishes crates in the
dependency order encoded by `scripts/publish-crates.sh`.
