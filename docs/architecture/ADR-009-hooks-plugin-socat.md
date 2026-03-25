# ADR-009: Claude Hooks via Plugin + Shell Script + socat

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

Seemux needs to receive real-time Claude Code lifecycle events (session-start, stop, notification, etc.) from terminal processes. Claude Code's plugin system supports registering shell command hooks with JSON payloads on stdin and a 10-second async timeout.

## Decision

A Claude Code plugin (`seemux-hooks`) registers shell hooks for all lifecycle events. Each hook invokes a bash script that reads JSON from stdin, wraps it with event metadata, and sends it to the seemux Unix socket via `socat`. The script is a no-op when `$SEEMUX_SOCKET` is unset.

## Consequences

### Positive

- Works within Claude Code's hook execution model (shell commands only)
- No compilation needed for the plugin — pure shell scripts
- `socat` is ubiquitous on Linux systems
- No-op outside seemux terminals (safe to install globally)
- Async hooks don't block Claude Code

### Negative

- Requires `socat` as a runtime dependency
- Each hook pays process spawn + socat connection cost
- JSON envelope constructed via `printf` in bash (fragile for edge cases)

## Alternatives Considered

### Direct library embedding

- **Description**: Embed a client library in Claude Code
- **Why rejected**: Not possible given Claude Code's hook execution model

### Named pipes

- **Description**: Use FIFO for event delivery
- **Why rejected**: Doesn't support concurrent writes cleanly

### D-Bus

- **Description**: Send events via D-Bus signals
- **Why rejected**: More complex setup, heavier dependency
