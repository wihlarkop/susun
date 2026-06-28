# ADR 040: Watch and Develop Architecture

## Status

Accepted for Phase 5 Workstream G.

## Context

Phase 5 adds an optional watch/develop subset for rebuild, restart, sync, and
sync-and-restart workflows. The implementation needs native file notifications,
debouncing, ignore rules, project-root confinement, cancellation, and runtime
integration without making watch support mandatory for library users.

The evaluated options were:

- implement directly inside `susun-cli`;
- add a neutral optional `susun-watch` crate backed by `notify`;
- use an external process watcher such as `watchexec`;
- defer watch support until after the compatibility program.

Direct CLI implementation would mix OS event handling, ignore evaluation, and
runtime actions into a command surface that should stay thin. An external
process watcher would reduce implementation work, but it would make normalized
events, confinement, cancellation, and library embedding harder to expose as
stable Susun behavior. Deferring would leave the Phase 5 watch/develop target
without an adapter boundary.

## Decision

Phase 5 introduces an optional `susun-watch` crate. The crate owns normalized
file events, debounce policy, root confinement, and ignore evaluation. The first
adapter uses the Rust `notify` crate for native OS file notifications.

`susun-watch` exposes Susun-owned event and action types only. It does not leak
`notify` event structs into `susun-model`, `susun-runtime`, or CLI contracts.
Runtime integration consumes normalized watch events and maps declared develop
actions to existing build, convergence, restart, and copy/sync operations.

The first supported subset is:

- rebuild;
- restart;
- sync files;
- sync files and restart.

All watched paths are resolved under the project root before observation and
before sync. Events whose normalized paths escape the root are rejected. Ignore
rules use the existing Dockerignore-style matcher where applicable, with
watch-specific exclusions layered above it. Debounce is deterministic at the
Susun event layer so callers do not depend on OS-specific notification bursts.

## Consequences

- Watch/develop support stays optional and capability-reported.
- Native OS watcher behavior is isolated behind Susun-owned normalized events.
- CLI and embedding users can share the same cancellation and debounce behavior.
- Symlink and path traversal checks are required before any sync action writes
  into a container or host path.
- The initial implementation can support a conservative subset while leaving
  room for richer Compose `develop` behavior later.
