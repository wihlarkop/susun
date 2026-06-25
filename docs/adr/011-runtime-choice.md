# ADR 011: Runtime Choice

## Status

Accepted for Phase 3.

## Context

Bollard is asynchronous and integrates naturally with Tokio. Phase 1 and Phase 2
must stay independent from any async runtime.

## Decision

Tokio is used only by the runtime-facing binary and Bollard adapter crates.
`susun-runtime` exposes runtime-agnostic futures and uses Tokio only for bounded
task scheduling and cancellation primitives. The facade and CLI opt into Tokio
when executing Docker operations.

## Consequences

- Phase 1 loading and Phase 2 planning remain synchronous.
- Adapter and CLI code can use the same async runtime as Bollard.
- Future adapters can implement `ContainerEngine` without exposing their own
  runtime types to callers.
