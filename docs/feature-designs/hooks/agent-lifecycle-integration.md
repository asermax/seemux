# Design: Agent Lifecycle Hook Integration

**Feature Spec**: [../../feature-specs/hooks/agent-lifecycle-integration.md](../../feature-specs/hooks/agent-lifecycle-integration.md)
**Status**: Current

## Purpose

This document explains the design rationale and architecture for Seemux's generic agent lifecycle hook integration. It covers the four-stage event pipeline, the JSON-RPC 2.0 NDJSON protocol, the polling-based channel consumption, notification classification, active-tab suppression, and the synchronous command API.

## Design Overview

The integration spans two process boundaries and operates through a four-stage pipeline:

1. **Extension/Plugin Layer** — Maps provider-specific signals into the canonical JSON-RPC 2.0 NDJSON contract and writes them to the Unix socket.
2. **Transport Layer** — `HookServer` background thread accepts Unix socket connections, parses JSON-RPC envelopes, and routes notifications/commands via an mpsc channel.
3. **Routing Layer** — GTK main thread polls the mpsc channel every 100ms, intercepts special application commands, applies suppression checks, and updates session metadata.
4. **Effect Layer** — `HookHandler` maps canonical methods to status mutations; `NotificationStore` registers badges; and command handlers execute terminal CRUD.

## Components

| Component | File | Responsibility |
|-----------|------|----------------|
| HookServer | `src/notifications/hook_server.rs` | Socket lifecycle, connection thread spawning, JSON-RPC envelope parsing, command blocking |
| HookHandler | `src/notifications/hook_handler.rs` | Stateless canonical event method → status/notification mapping |
| Hook Polling | `src/app/hooks/mod.rs` | 100ms GLib polling, special method interception, post-stop suppression, stale PID detection |
| Command Handler | `src/app/hooks/commands.rs` | Session/group CRUD executed via synchronous socket command requests |
| NotificationStore | `src/notifications/mod.rs` | Per-session unread badge counts, change callbacks |

## Modeling

### Session Status State Machine

```
         agent.session.started   agent.prompt.submitted / agent.tool.pre_use
[Any] ─────────────────────────> Idle ──────────────────────────────────────────> Running
                                   ^                                                 |
         agent.response.completed/ |                     agent.attention.requested   |
         agent.session.ended       |                                                 v
                                   |                                            NeedsInput
                                Running
                                   |
             agent.response.failed v
                                 Error
```

## Key Decisions

### JSON-RPC 2.0 over Newline-Delimited Socket

**Choice**: Adopt standard JSON-RPC 2.0 NDJSON schemas for both fire-and-forget hook event notifications and synchronous Command API requests.
**Why**:
- Standardizes both notifications (has `"method"` but lacks `"id"`) and requests (has `"method"` and `"id"`) at the schema level.
- Provides standard error modeling (`{"code": ..., "message": ...}`) and protocol envelopes, making it easy to parse and implement across various client runtimes (Bash, Node.js).
- Retains NDJSON framing, allowing single-threaded line-by-line buffering.

### Polling-Based Channel Consumption (100ms)

**Choice**: GTK main thread polls the `mpsc::Receiver` every 100ms via `glib::timeout_add_local`.
**Why**:
- Keeps Seemux's main loop completely single-threaded and avoids pulling in heavy async runtimes.
- Decouples blocking socket I/O from the UI thread while preventing race conditions on shared memory structures (`SessionManager`).

### Keyword-Based Notification Classification

**Choice**: Priority-ordered, case-insensitive keyword matching (Permission > Error > Completed > Waiting > Attention).
**Why**:
- Unified, robust heuristic that bridges different provider vocabularies (Claude, Pi, or custom scripts) to present consistent unread notification severity levels to the user.
