# ADR-003: Unix Socket as IPC Mechanism

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

Seemux needs IPC for: (1) Claude Code hook events from terminal processes, (2) CLI toggle commands from external processes, (3) programmatic session/group CRUD from the tmux shim, (4) tray icon click events from the ksni thread. All need to reach the GTK main thread.

## Decision

Use a single Unix domain socket at `$XDG_RUNTIME_DIR/seemux/seemux.sock` with newline-delimited JSON protocol. Hook events and commands share the same socket, discriminated by the presence of `request_id`/`command` fields. A background thread accepts connections and forwards messages via `mpsc` channel to the main thread.

## Consequences

### Positive

- Single IPC endpoint for all external communication
- JSON protocol is simple to produce from shell scripts (via `socat`)
- Unix sockets are reliable, ordered, and support concurrent connections
- Reusable by multiple producers (hooks, CLI, tray, tmux shim)

### Negative

- Requires `socat` as runtime dependency for hook scripts
- Command responses have up to 100ms latency (polling interval)
- Socket file must be cleaned up on startup (stale) and shutdown (Drop)

## Alternatives Considered

### D-Bus

- **Description**: Standard Linux IPC bus
- **Why rejected**: Heavier setup, requires service registration, overkill for single-application IPC

### Named pipes (FIFO)

- **Description**: POSIX named pipes for message passing
- **Why rejected**: Not suitable for concurrent writers; no request/response semantics

### HTTP/REST

- **Description**: Local HTTP server for API access
- **Why rejected**: Excessive overhead for local IPC; requires port allocation
