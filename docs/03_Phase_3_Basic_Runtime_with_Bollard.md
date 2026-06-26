ComposeKit

Phase 3 - Basic Runtime with Bollard

Execute plans against Docker Engine through the first adapter and support a practical Compose subset.


| Document | Value |

| --- | --- |

| Project | ComposeKit |

| Status | Complete / Implemented in Phase 3 |

| Primary language | Rust |

| Architecture | Library-first, adapter-based, testable by construction |

| Audience | Maintainers, contributors, implementers, reviewers |

| Version | 1.0 |



## 1. Objective

Provide working up, down, start, stop, restart, ps, and logs operations while preserving the planner/runtime separation.

## Implementation Status

Phase 3 is implemented in Susun through the neutral runtime and Bollard adapter crates. The phase branch completed with green CI for formatting, clippy, rustdoc warnings, MSRV/stable tests, feature combinations, and platform tests on macOS, Ubuntu, and Windows.

## 2. Why This Phase Exists

This phase is the next bounded step in the ComposeKit roadmap. It must produce a usable, testable release while keeping later phases possible. Work that belongs to later phases is intentionally excluded even when it appears convenient.

## 3. Deliverable Crates

| Crate / subproject | Responsibility |

| --- | --- |

| composekit-engine-bollard | Translation between neutral engine contracts and Bollard. |

| composekit-runtime | Plan execution, cancellation, progress events, retries, and result collection. |

| composekit-cli | Operational commands and terminal progress. |


## 4. Functional Scope

### 3A. Engine connectivity

- Local Unix socket, Windows named pipe where supported, and configured TCP/TLS.

- Version and capability discovery.

- Clear connection diagnostics.

### 3B. Resource operations

- Pull images.

- Create and inspect networks and volumes.

- Create, start, stop, and remove containers.

- Map ports, mounts, environment, commands, healthchecks, and restart policy for the supported subset.

### 3C. Plan executor

- Execute dependency-ready actions concurrently within limits.

- Propagate cancellation.

- Capture action results and errors.

- Stop dependent actions after failure.

- Support progress event subscribers.

### 3D. Runtime commands

- up, down, start, stop, restart, ps, logs.

- Service selection.

- Detach mode.

- Timeout configuration.

### 3E. Safety

- Redact secrets.

- Require force for destructive conflicts.

- Record partial completion.

- Make retries idempotent where possible.

## 5. Core API Sketch

```text
let engine = BollardEngine::local()?;
let runtime = Runtime::new(engine);
let plan = runtime.plan_up(&project, UpOptions::default()).await?;
let result = runtime.apply(plan).await?;
```

## 6. Data and Error Contracts

- Public models are serializable where doing so is safe and meaningful.

- Stable diagnostic and action codes are documented.

- Errors retain causal chains for library callers and concise summaries for CLI users.

- Secrets and credentials are represented with redacting wrappers.

- Unsupported capability errors are distinct from invalid input errors.

## 7. Testing Strategy

- Unit tests for every normalization, validation, planning, or translation rule.

- Golden fixtures for stable diagnostics and serialized outputs.

- Property tests for parsers, naming, ordering, and idempotency where appropriate.

- Compatibility fixtures derived from valid and invalid Compose projects.

- Integration tests use ephemeral resources and clean them by project label.

- Failure-path tests cover cancellation, partial execution, and cleanup.

## 8. Acceptance Criteria

- A supported image-based project can be brought up and down through the Rust API without invoking docker compose.

- Dependency ordering and service_healthy waits work for the supported subset.

- Runtime executes an immutable plan and emits progress events.

- Failures produce partial-result reports and do not silently continue dependent work.

- Integration tests run against a real Docker daemon in CI.

## 9. Explicitly Out of Scope

- Selective recreation based on full semantic hashes.

- Scaling beyond one replica.

- BuildKit builds.

- Watch/develop mode.

- Perfect CLI output parity.

## 10. Work Packages

| Package | Output | Depends on |

| --- | --- | --- |

| 3.1 Foundations | Public models, errors, and fixture conventions | Previous phase APIs |

| 3.2 Core implementation | Primary phase behavior and unit tests | 3.1 |

| 3.3 CLI and serialization | Commands, JSON schema, examples | 3.2 |

| 3.4 Compatibility suite | Oracle fixtures and regression tests | 3.2 |

| 3.5 Release hardening | Docs, benchmarks, security review, migration notes | 3.3 and 3.4 |


## 11. Exit Gate

1. All acceptance criteria are represented by automated tests.

1. The capability matrix is updated.

1. Examples compile and execute in CI.

1. Public APIs have rustdoc and at least one end-to-end example.

1. Known compatibility gaps are documented rather than hidden.

1. The next phase can consume this phase through public contracts without copying internals.
