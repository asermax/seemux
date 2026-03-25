# Design: Claude Code Hook Integration

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/hooks/claude-code-integration.md](../../feature-specs/hooks/claude-code-integration.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/notifications/`, `src/app/hooks/`, `plugins/seemux-hooks/`.
Retrofit date: 2026-03-24
Decisions discovered: Unix socket with shell script bridge (ADR), Socket command multiplexing (DES)

---

## Purpose

This document explains the design rationale for the Claude Code hook integration: the four-stage event pipeline, notification classification, suppression logic, command API, and stale PID detection.

## Problem Context

Users run multiple Claude Code sessions simultaneously. Without integration, they must manually check each tab for status. The system needs to capture Claude lifecycle events from inside terminal processes, deliver them to the host process, and map them to visual indicators — all without blocking Claude Code.

## Design Overview

A four-stage pipeline spanning two process boundaries:

1. **Plugin layer** — Shell hook scripts read JSON from stdin, wrap with metadata, deliver via `socat` to Unix socket
2. **Transport layer** — `HookServer` background thread accepts connections, parses JSON, routes via mpsc channel
3. **Routing layer** — GTK main thread polls channel every 100ms, applies suppression, delegates to handler/commands
4. **Effect layer** — `HookHandler` maps to status transitions; command dispatcher executes CRUD; `NotificationStore` tracks unreads

## Components

| Component | File | Responsibility |
|-----------|------|----------------|
| seemux-hooks plugin | `plugins/seemux-hooks/` | Hook registration, shell script bridge |
| HookServer | `notifications/hook_server.rs` | Socket lifecycle, connection threading, message routing |
| HookHandler | `notifications/hook_handler.rs` | Stateless event → status/notification mapping |
| Hook polling | `app/hooks/mod.rs` | 100ms timer, suppression logic, stale PID detection |
| Command dispatcher | `app/hooks/commands.rs` | Session/group CRUD via socket commands |
| NotificationStore | `notifications/mod.rs` | Per-session unread counts, change callback |

## Modeling

### Session Status State Machine

```
         session-start           prompt-submit / pre-tool-use
[Any] ─────────────────> Idle ────────────────────────────────> Running
                           ^                                      |
                    stop / |                    notification       |
                session-end|                                      v
                           |                                 NeedsInput
                        Running
                           |
             stop-failure  v
                        Error
```

Stale PID detection: any status with PID → Idle when process dead.

## Data Flow

### Hook Event (happy path)

Claude hook fires → shell script wraps JSON → `socat` to socket → per-connection thread parses → mpsc channel → 100ms poll → suppression check → `handle_hook_event` → status update + optional notification → sidebar badge update.

### Command Request/Response

External tool sends JSON with `request_id` + `command` → connection thread blocks on `SyncSender` → 100ms poll dispatches → handler executes → response sent back → connection thread writes to socket.

### Notification Suppression

- **Active session**: notifications for the focused tab are discarded
- **Post-stop**: `stopped_sessions` HashSet tracks sessions after stop/stop-failure; notifications for these are dropped until a new turn begins (prompt-submit, pre-tool-use, session-start, session-end)

## Key Decisions

### Unix Socket with Shell Script Bridge

**Choice**: Shell script + `socat` → Unix socket, rather than direct IPC or embedded client.
**Why**: Claude Code hooks only support shell commands. `socat` is ubiquitous. Script is a no-op when `$SEEMUX_SOCKET` unset.
**Consequences**: Requires `socat` runtime dependency. Each hook pays process spawn cost.

### Polling-Based Channel Consumption (100ms)

**Choice**: `glib::timeout_add_local` polling mpsc receiver.
**Why**: Avoids async runtime dependency. Simple producer-consumer pattern.
**Consequences**: Up to 100ms latency. Burst of events drained in single loop iteration.

### Keyword-Based Notification Classification

**Choice**: Priority-ordered keyword matching (Permission > Error > Completed > Waiting > Attention).
**Why**: Claude Code payloads lack structured severity fields. Keyword heuristic is resilient across payload variations.
**Consequences**: Fragile to false positives. Adding categories requires code changes.

### Synchronous Command API on Same Socket

**Choice**: Commands distinguished by `request_id`/`command` fields. Connection thread blocks until main thread replies.
**Why**: Reuses existing socket. Clean synchronous API without second IPC endpoint.
**Consequences**: Command round-trip includes up to 100ms polling latency.

## System Behavior

### Post-Stop Suppression

`stop` received for session B → B added to `stopped_sessions` → late `notification` for B → silently dropped. `prompt-submit` for B → B removed from set → future notifications processed normally.

### Stale PID Recovery

5-second timer → `kill(pid, 0)` for all tracked PIDs → dead PID: clear PID, clear session ID, status → Idle.

---

## Notes

- `HookResult.claude_pid` uses `0` as sentinel for "clear PID" — inconsistent with `claude_session_id` which uses `Option<Option<String>>`.
- `post-tool-use` only consumed for PR detection side effect, not status transitions.
- `stopped_sessions` set never garbage-collected for destroyed sessions.
