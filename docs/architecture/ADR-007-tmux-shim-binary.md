# ADR-007: Tmux Shim as Separate Binary

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

Claude Code Agent Teams uses tmux as its multiplexer backend, invoking `tmux` as a CLI command. Seemux needs to intercept these calls and translate them into native session/group operations.

## Decision

Build a standalone Rust binary (`seemux-tmux-shim`) deployed as `tmux` in a runtime bin directory prepended to `$PATH`. The shim intercepts when `$SEEMUX_SOCKET` is set and falls through to `/usr/bin/tmux` otherwise. The feature is gated behind the `agent_teams_shim` config flag.

## Consequences

### Positive

- Transparent to Agent Teams — no Agent Teams configuration changes needed
- Falls through to real tmux outside seemux
- File-locked pane map handles concurrent agent access safely
- Feature-gated — disabled by default

### Negative

- Must build and distribute alongside the main binary
- PATH modification affects all child processes
- Hardcoded fallthrough path `/usr/bin/tmux`
- Two-phase creation (pending pane → actual session) adds complexity

## Alternatives Considered

### Shell script wrapper

- **Description**: A bash script that intercepts tmux commands
- **Why rejected**: Slower startup, harder to manage file locking and JSON parsing

### Modifying Agent Teams to use a different binary

- **Description**: Configure Agent Teams to use `seemux` instead of `tmux`
- **Why rejected**: Agent Teams hardcodes `tmux`; no configuration option available
