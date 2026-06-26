# ADR 010: Async Engine Traits

## Status

Accepted for Phase 3.

## Context

The planner remains synchronous and daemon-independent, but runtime execution
must call asynchronous engine clients. Engine adapters must be swappable without
exposing Docker-specific types through neutral crates.

## Decision

`susun-engine` exposes an object-safe `ContainerEngine` trait using boxed
`Future` values and boxed neutral log streams. This avoids a public dependency
on the `async-trait` macro while preserving `Arc<dyn ContainerEngine>` support.

The trait requires `Send + Sync` engines and `Send` futures. Adapter-specific
client/runtime details remain in adapter crates.

## Consequences

- Neutral crates do not expose Bollard or Tokio types.
- Runtime scheduling can store engines behind `Arc<dyn ContainerEngine>`.
- Implementations are slightly more verbose than native async trait methods.
- If native async trait object safety becomes stable and ergonomic under the
  project MSRV, the boxed trait can be migrated behind a compatibility layer.
