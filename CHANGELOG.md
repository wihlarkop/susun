# Changelog

## Unreleased

- Added Phase 13 runtime-readiness DTOs for connection profiles and redacted
  doctor reports, plus a Bollard-backed profile doctor helper.
- Added `susun doctor` to emit a redacted local runtime readiness report.
- Added runtime profile selection contracts for default-profile validation and
  duplicate ID detection.
- Hardened runtime profile serde deserialization so invalid profile IDs,
  duplicate IDs, and multiple defaults are rejected at the typed boundary.
- Added facade JSON helpers for parsing and rendering runtime profile sets.
- Added runtime status summary DTOs and JSON helpers derived from neutral engine
  snapshots.
- Added `susun status` for human and JSON runtime status summaries.
- Added runtime overview DTOs and JSON helpers that combine runtime readiness
  with optional project status.
- Added `susun overview` for combined runtime readiness and project status
  output.

## 0.1.0

- First public release candidate for Susun.
- Expanded the Phase 1 canonical model with Compose resources, service
  references, dependencies, healthchecks, restart policies, and profiles.
- Added active profile selection, semantic validation, and deterministic
  dependency graph construction.
- Added JSON diagnostic rendering behind the `susun` facade and completed the
  basic CLI flags for diagnostic format, quiet mode, and color policy.
- Completed the Phase 2 through Phase 5 public surface: neutral planning,
  Docker runtime operations, convergence, build compatibility, advanced Compose
  loading, watch support, and compatibility reporting.
- Added Phase 7 release-hardening evidence: real-world compatibility catalog,
  release-readiness manifest, generated compatibility docs, and release gates.
- Added Phase 8 SDK readiness surface: `SusunWorkspace`, `SdkProject`,
  serializable project/service summaries, daemon-free dry-run helpers, and the
  `susun summary` CLI command, plus curated facade re-exports for common SDK
  model, planner, engine, and runtime types.
- Added Phase 9 CLI and SDK consumer readiness: versioned project summary JSON,
  project-summary schema, richer resource summaries, SDK example, and
  consumer-readiness gates.
- Added Phase 12 release-candidate audit checks: publish-facing package
  metadata, dual-license files, versioned internal dependencies, package
  assembly checks, and final release-candidate gates.
- Replaced the placeholder README with Phase 1 usage and scope documentation.
