# ADR 030: BuildKit Transport

## Status

Accepted for Phase 5 Workstream B.

## Context

Phase 5 needs build execution without leaking Docker, BuildKit, buildx, or
registry implementation types into Susun's canonical model, planner, or runtime
contracts. The project already has a neutral engine boundary for container
operations, so build execution should follow the same pattern.

The evaluated options were:

- Docker Engine BuildKit API through a Rust client;
- native BuildKit client transport;
- `docker buildx build` process adapter;
- deferring all execution until a later phase.

The native and Docker Engine API paths are better long-term integrations, but
they require more protocol surface, authentication handling, progress decoding,
and maintenance work than this workstream should absorb. Deferring execution
would leave Phase 5 without a runnable adapter boundary.

## Decision

Phase 5 introduces a neutral `BuildEngine` contract in `susun-build` and selects
an intentionally narrow `docker buildx build` process adapter as the first
transport.

The process adapter is allowed to shell out only behind the neutral build
contract. It must not become part of the canonical model, planner schema, or
runtime public contracts. Future native transports can implement the same
contract.

The adapter uses plain progress mode and translates process lifecycle/log output
into neutral build events. Rich BuildKit vertex decoding is deferred until a
native or structured transport is selected.

## Consequences

- Users can exercise supported builds with an installed Docker CLI and buildx.
- Public contracts remain backend-independent.
- Cancellation is cooperative before/after process execution in this iteration;
  hard process-tree termination can be added when runtime integration owns
  process supervision.
- Credentials and secret values remain provider-owned and must not serialize
  into plans, reports, snapshots, or debug output.
- The adapter is replaceable; BuildKit-specific request/response types do not
  leak into `susun-model`.
