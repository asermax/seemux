# Design: Tmux Shim -- Agent Teams Compatibility

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/agent-teams/tmux-shim.md](../../feature-specs/agent-teams/tmux-shim.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/bin/seemux_tmux_shim.rs`, `src/runtime.rs`.
Retrofit date: 2026-03-24
Decisions discovered: PATH interception (ADR), File-locked pane map (ADR), Deferred session creation / pending pane pattern (DES)

---

## Purpose

This document explains the design rationale for the tmux shim: command interception, the two-phase session creation pattern, concurrent pane map management, and graceful degradation for unsupported commands.

## Problem Context

Claude Code Agent Teams uses tmux to spawn teammate agents. Seemux has no tmux compatibility. The shim intercepts tmux CLI invocations, translating the subset Agent Teams uses into seemux socket commands. Must handle concurrent access from multiple agents and fall through to real tmux outside seemux.

## Design Overview

Three components:

1. **Shim binary** (`seemux-tmux-shim`) — Deployed as `tmux` in runtime bin dir, intercepting CLI calls
2. **Runtime deployment** (`runtime.rs`) — Symlink/copy deployment, PATH injection
3. **Socket command handlers** (`app/hooks/commands.rs`) — Server-side session/group CRUD

Activation chain: config flag → deployment at startup → PATH prepended in spawned shells → shim found before system tmux.

## Modeling

```
Pane Map (pane-map.json, file-locked)
├── "%0" → "__lead__"      (originating session)
├── "%1" → "__pending__"   (allocated, not yet created)
├── "%2" → "uuid-..."      (resolved to seemux session)
└── "%N" → ...
```

Three sentinel values: `__lead__` (orchestrator pane), `__pending__` (allocated but unactivated), real UUID (active session).

## Data Flow

### Teammate Creation (two-phase)

1. `split-window -P` → allocate pane ID as `__pending__` in map → print ID
2. `send-keys -t %N "cd /path && claude --team-name foo --agent-name writer" Enter` → detect Claude launch command → `create-group "Team: foo"` → `create-session` with title/argv/cwd → update map with real session UUID

### Raw Input Forwarding

`send-keys -t %N "text" Enter` for resolved pane → look up UUID → `send-input` via socket.

### Teardown

`kill-pane -t %N` → remove from map → if real UUID: `destroy-session` via socket.

### Fallthrough

`$SEEMUX_SOCKET` unset → pass all args to `/usr/bin/tmux`.

## Key Decisions

### Separate Binary with PATH Interception

**Choice**: Standalone Rust binary deployed as `tmux`, PATH prepended in spawned shells.
**Why**: Agent Teams hardcodes `tmux`. No way to configure a different binary name.
**Alternatives**: Shell wrapper (slow, fragile), LD_PRELOAD (complex).
**Consequences**: Must build/distribute alongside seemux. PATH modification affects all child processes.

### Deferred Session Creation (Pending Pane Pattern)

**Choice**: `split-window` allocates ID with `__pending__`; session created on `send-keys` with Claude command.
**Why**: Agent Teams splits first, then sends command. Can't know session details at split time.
**Consequences**: Brief window where pane ID exists but no session. Non-Claude commands to pending panes silently dropped.

### File-Locked JSON Pane Map

**Choice**: JSON file with exclusive `flock` for concurrent access.
**Why**: Multiple shim processes run concurrently with no shared memory. File locking is simplest correct mechanism.
**Alternatives**: SQLite (heavyweight), shared memory (complex), socket-based state (round-trips).
**Consequences**: Read-modify-write per invocation. Negligible for <10 concurrent agents.

### Graceful No-Op for Unsupported Commands

**Choice**: Layout/selection commands return success silently. Unknown commands log to stderr and succeed.
**Why**: Agent Teams sends many commands with no seemux equivalent. Failing would break the workflow.
**Consequences**: If Agent Teams starts relying on output from nooped commands, shim needs updates.

### Command Detection via String Matching

**Choice**: Check if `send-keys` text contains `"claude"` and `"--team-name"`.
**Why**: No structured API from Agent Teams — raw command strings only.
**Consequences**: Fragile to CLI flag renames. `extract_flag_from_command` strips backslashes for escaped values.

## System Behavior

### Concurrent Access

Three agents invoke shim simultaneously → each acquires exclusive `flock` on `pane-map.json` → serialized access → consistent snapshots.

### Feature Disabled

`agent_teams_shim: false` → no deployment, PATH unmodified, `tmux` resolves to system binary.

---

## Notes

- Hardcoded fallthrough path `/usr/bin/tmux` may not work on all systems.
- Pane map never explicitly cleaned up on seemux exit (relies on `$XDG_RUNTIME_DIR` cleanup).
- Duplicated `runtime_dir()` function between main process and shim binary — must stay in sync manually.
- `seemux-env` subcommand generates shell export/unset statements for `$TMUX` variable.
