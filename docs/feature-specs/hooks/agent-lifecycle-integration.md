# Agent Lifecycle Hook Integration Spec

## Overview

Seemux provides a unified, provider-neutral agent lifecycle integration contract over a newline-delimited JSON (NDJSON) Unix socket. It adopts the standard JSON-RPC 2.0 protocol over the socket to handle both fire-and-forget lifecycle event notifications from active agent sessions and synchronous request-response command APIs from external tools or scripts.

This specification describes the canonical event contract, the status mapping rules, notification classification, notification suppression rules, stale PID cleanup, and the socket commands API.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Receive real-time lifecycle events from active agent sessions and reflect them as visual session statuses on the tab |
| R1 | Unix socket server runs in a background thread, forwarding parsed messages to the GTK main thread via an mpsc channel |
| R2 | Modern JSON-RPC 2.0 socket parser rejects messages that do not conform to the protocol version |
| R3 | Hook handler maps canonical lifecycle methods to session status transitions and optional unread notification badges |
| R4 | Notifications are only generated for background (non-active) tabs; in dropdown mode, active-tab notifications are suppressed only when the dropdown window is visible |
| R5 | Stale notifications that arrive after a stop/stop-failure for the same turn are suppressed until a new turn begins |
| R6 | NotificationStore tracks per-session unread counts and latest notification body, emitting a change callback on mutation |
| R7 | Stale PID detection runs every 5 seconds, resetting session status to Idle when a tracked process has exited |
| R8 | Command API supports list-sessions, focus-session, send-input, create-session, create-group, and destroy-session via synchronous JSON-RPC 2.0 requests |
| R9 | Special application-level controls (toggle-dropdown, activate-window, quit) are handled as application-wide triggers |
| R10 | Socket file is automatically cleaned up on startup (stale) and shutdown (Drop) |

## Behaviors

### Socket Message Routing

**Acceptance Criteria**:
- Given an inbound NDJSON line, when parsed as a valid JSON-RPC 2.0 message with an `"id"` field, then it is routed as a Command Request, and the server thread blocks waiting for the main thread response.
- Given an inbound NDJSON line, when parsed as a valid JSON-RPC 2.0 message without an `"id"` field, then it is routed as an Event Notification.
- Given any inbound message that is invalid JSON or lacks `"jsonrpc": "2.0"`, then it is rejected, logged to stderr, and discarded.

### Session Status Transitions

**Acceptance Criteria**:
- Given a session, when `agent.session.started` is received, then status transitions to `Idle`, unread notifications are cleared, and the agent's PID, session ID, provider name, and binary name are recorded.
- Given a session, when `agent.prompt.submitted` or `agent.tool.pre_use` is received, then status transitions to `Running` and unread notifications are cleared.
- Given a session, when `agent.attention.requested` is received, then status transitions to `NeedsInput` with a classified notification body.
- Given a session, when `agent.response.completed` is received, then status transitions to `Idle` with a truncated 100-character notification preview.
- Given a session, when `agent.response.failed` is received, then status transitions to `Error` with a truncated 100-character notification preview.
- Given a session, when `agent.session.ended` is received, then status transitions to `Idle` and its active agent PID, session ID, and binary name are cleared.

### Notification Classification

**Acceptance Criteria**:
- Given payload text containing "permission", "approve", or "approval", then classified as `"Permission"` and mapped to the fallback body `"Approval needed"`.
- Given text containing "error", "failed", or "exception", then classified as `"Error"` and mapped to the fallback body `"Agent reported an error"`.
- Given text containing "complet", "finish", "done", or "success", then classified as `"Completed"` and mapped to the fallback body `"Task completed"`.
- Given text containing "idle", "wait", or "input", then classified as `"Waiting"` and mapped to the fallback body `"Waiting for input"`.
- Given no recognized keywords, then classified as `"Attention"` and mapped to the fallback body `"Agent needs your attention"`.

### Notification Suppression

**Acceptance Criteria**:
- Given the active session, when a hook produces a notification for it, then the notification is discarded (unless the dropdown window is hidden, in which case the badge is shown).
- Given a stop/stop-failure was received for session X, when a subsequent late notification arrives for X, then it is discarded until a new turn begins.

### Stale PID Detection

**Acceptance Criteria**:
- Given a session with a recorded agent PID, when the 5-second timer fires and the process is dead, then its PID, session ID, and binary name are cleared, and its status returns to `Idle`.

### Command API

**Acceptance Criteria**:
- Given `create_group` with a `name`, when a matching group exists, then the existing group ID is returned; otherwise a new group is created.
- Given `create_session` with optional title/cwd/group_id/argv, then a new session is created and its ID is returned.
- Given `destroy_session` with a `session_id`, then the session is destroyed.
- Given `focus_session` with a `session_id`, then the session is activated.
- Given `list_sessions` with optional `group_id`, then session IDs are returned in sidebar order.
- Given `send_input` with `session_id` and `text`, then the text is fed to the VTE terminal.
