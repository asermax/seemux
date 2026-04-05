# Claude Code Hook Integration

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/notifications/`, `src/app/hooks/`, `plugins/seemux-hooks/`.
Retrofit date: 2026-03-24

---

## Overview

End-to-end event pipeline between Claude Code and Seemux. A Claude Code plugin registers hooks for eight lifecycle events, shell scripts forward JSON via socat to a Unix socket, a background thread receives and routes messages via mpsc to the GTK main thread, and a handler maps events to session status transitions and notifications. The same socket also provides a command API for programmatic session/group control.

## User Stories

- As a developer running Claude Code sessions inside Seemux, I want real-time lifecycle status reflected on each tab so I can see which sessions are idle, running, waiting for input, errored, or completed without switching tabs.
- As a developer with background sessions, I want notifications when sessions need attention so I never miss important events.
- As an external tool or script, I want to programmatically create/destroy/focus sessions and send input via a socket API.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Receive real-time Claude Code lifecycle events and reflect them as visual session status on each tab |
| R1 | Unix socket server runs in a background thread, forwarding messages to GTK main thread via mpsc |
| R2 | Hook handler maps event types (session-start, prompt-submit, pre-tool-use, notification, stop, stop-failure, session-end) to status transitions and optional notifications |
| R3 | Notifications are only generated for background (non-active) sessions |
| R4 | Stale notifications after stop/stop-failure for the same turn are suppressed until a new turn begins |
| R5 | NotificationStore tracks per-session unread counts and latest notification body, emitting a change callback on mutation |
| R6 | Stale PID detection runs every 5 seconds, resetting status to Idle when a tracked process has exited |
| R7 | Hook script is a no-op when `$SEEMUX_SOCKET` is unset |
| R8 | Plugin registers hooks for all eight Claude Code lifecycle events, each async with 10-second timeout |
| R9 | Command API supports create-group, create-session, destroy-session, focus-session, list-sessions, and send-input with JSON request/response |
| R10 | Special non-session events (toggle-dropdown, activate-window, quit) are handled as application-level controls |
| R11 | post-tool-use events for Bash tool containing `git` or `gh` commands trigger branch/PR re-detection |
| R12 | Socket file is cleaned up on startup (stale) and shutdown (Drop) |

## Behaviors

### Event Delivery

**Acceptance Criteria**:
- Given a Claude Code session running inside Seemux with `$SEEMUX_SOCKET` and `$SEEMUX_SESSION_ID` set, when any hook fires, then the script wraps the JSON payload and sends it to the socket
- Given `$SEEMUX_SOCKET` is unset, when any hook fires, then the script exits immediately with code 0

### Socket Message Routing

**Acceptance Criteria**:
- Given a JSON message with `request_id` and `command` fields, when received, then it is routed as a Command and the server blocks until the main thread replies
- Given a JSON message without those fields, when received, then it is deserialized as a HookEvent
- Given invalid JSON, when received, then it is logged to stderr and discarded
- Given a blank line, when received, then it is silently ignored

### Session Status Transitions

**Acceptance Criteria**:
- Given a session, when `session-start` is received, then status becomes Idle, notifications are cleared, and Claude PID/session ID are recorded
- Given a session, when `prompt-submit` is received, then status becomes Running and notifications are cleared
- Given a session, when `pre-tool-use` is received, then status becomes Running and notifications are cleared
- Given a session, when `notification` is received, then status becomes NeedsInput with a classified notification body
- Given a session, when `stop` is received, then status becomes Idle with a notification from last_assistant_message (truncated to 100 chars)
- Given a session, when `stop-failure` is received, then status becomes Error with a notification from message/error/reason
- Given a session, when `session-end` is received, then status becomes Idle and Claude PID, session ID, and binary name are cleared

### Notification Classification

**Acceptance Criteria**:
- Given payload text containing "permission", "approve", or "approval", then classified as "Permission"
- Given text containing "error", "failed", or "exception" (no permission keywords), then classified as "Error"
- Given text containing "complet", "finish", "done", or "success" (no higher priority), then classified as "Completed"
- Given text containing "idle", "wait", or "input" (no higher priority), then classified as "Waiting"
- Given no recognized keywords, then classified as "Attention"

### Notification Suppression

**Acceptance Criteria**:
- Given the active session, when a hook produces a notification for it, then the notification is discarded
- Given a stop/stop-failure was received for session X, when a subsequent notification arrives for X, then it is discarded until a new turn begins

### Stale PID Detection

**Acceptance Criteria**:
- Given a session with a recorded Claude PID, when the 5-second timer fires and the process is dead, then PID, session ID, and Claude binary name are cleared, and status is set to Idle
- Given a session with a live PID, when the timer fires, then no changes are made

### NotificationStore

**Acceptance Criteria**:
- Given a notification is added for session X, then unread count increments and latest notification updates
- Given mark_read is called for session X, then unread count resets to 0 but latest notification is preserved
- Given clear_session is called for session X, then both unread count and latest notification are removed
- Given an on_change callback is set, when any mutation occurs, then the callback fires with session ID, new count, latest notification, and total

### Command API

**Acceptance Criteria**:
- Given `create-group` with a `name`, when a matching group exists, then the existing group ID is returned; otherwise a new group is created
- Given `create-session` with optional title/cwd/group_id/argv, then a new session is created and its ID is returned
- Given `destroy-session` with a `session_id`, then the session is destroyed
- Given `focus-session` with a `session_id`, then the session is activated
- Given `list-sessions` with optional `group_id`, then session IDs are returned in sidebar order
- Given `send-input` with `session_id` and `text`, then the text is fed to the VTE terminal
- Given an unknown command, then an error response is returned

### Socket Lifecycle

**Acceptance Criteria**:
- Given Seemux is starting, when the HookServer is constructed, then stale socket files are removed and the runtime directory is created
- Given the HookServer is dropped, when Drop runs, then the socket file is removed
