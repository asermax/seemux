# Session Management

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/session/mod.rs`, `src/session/manager.rs`.
Retrofit date: 2026-03-24

---

## Overview

The session domain is the central orchestration layer of Seemux. A "session" ties together a terminal (or split tree of terminals), a sidebar tab, an optional Claude Code agent, and lifecycle metadata. The SessionManager owns all sessions, coordinates creation/destruction/switching, manages split panes, wires VTE signal handlers for title/CWD/bell tracking, handles persistence, and provides navigation primitives (adjacent tab, adjacent group, jump-to-notification, jump-to-running).

## User Stories

- As a terminal user, I want to create, switch between, and close sessions so I can work on multiple tasks simultaneously.
- As a terminal user, I want to split panes horizontally/vertically and navigate between them so I can view multiple processes side by side.
- As a terminal user, I want each tab to show my folder name, git branch, PR number, and running command automatically.
- As a user managing many sessions, I want to organize them into named groups and navigate between groups.
- As a user who restarts the app, I want sessions restored including splits, CWDs, groups, and Claude session IDs.
- As a Claude Code user, I want sessions to track Claude's lifecycle status and resume sessions after restart.
- As a user with many tabs, I want to jump to sessions with unread notifications or active commands.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Manage full lifecycle of terminal sessions: create, switch, destroy, each identified by UUID and displayed as a sidebar tab |
| R1 | Sessions start in Idle status, assigned to "default" group unless specified |
| R2 | Sessions can be created with a shell, specific command, specific group, and optional CWD |
| R3 | When active session is destroyed, focus moves to: previous sibling, next sibling, first visible tab, or first tab in any group |
| R4 | "Close others" destroys all sessions except the specified one |
| R5 | When the last session is destroyed, an on_empty callback fires |
| R6 | Split focused pane horizontally or vertically, new pane inherits parent CWD |
| R7 | Close focused pane; if last pane, destroy the session |
| R8 | Navigate between panes directionally (up/down/left/right) |
| R9 | Track VTE title changes: detect shell prompts (revert to folder name) vs running commands (show command name) |
| R10 | Track VTE CWD changes: update folder/subtitle, trigger async git branch and PR detection |
| R11 | After git/gh commands complete, re-detect branch and PR with 2-second debounce |
| R12 | Sessions belong to groups; can be moved between groups |
| R13 | Circular tab switching: next/previous among visible sessions, next/previous group |
| R14 | Switch to specific tab by visible index (Ctrl+1..9) |
| R15 | Persist session state: ordered sessions with title, split tree, CWDs, group, Claude session ID, groups with collapsed state, active index |
| R16 | Restore sessions from saved state, deferring shell spawning for collapsed groups |
| R17 | Track Claude PID and session ID; store pending_resume_id for restored sessions |
| R18 | Expose take_pending_resumes for Claude session resume injection |
| R19 | Non-Claude sessions show "Running" pill with 3-second delay, reset to Idle on prompt return, emit completion notification for background tabs |
| R20 | Terminal bell events on background sessions produce debounced notifications (2s per session) |
| R21 | Navigate to next/previous session with unread notifications (circular) |
| R22 | Navigate to next/previous session with active status or a running command in any pane (circular) |
| R23 | Inject SEEMUX_SOCKET and SEEMUX_SESSION_ID env vars; optionally prepend bin dir to PATH |

## Behaviors

### Session Creation

**Acceptance Criteria**:
- Given no sessions exist, when a session is created with no title, then it receives "Tab 1", a UUID, Idle status, and "default" group
- Given 3 sessions exist, when a new session is created, then it receives "Tab 4"
- Given a specific CWD, when the shell spawns, then it starts in that directory
- Given a specific command, when the terminal spawns, then the command executes instead of an interactive shell
- Given the GTK stack is not realized, when created, then the shell spawn is deferred

### Session Destruction

**Acceptance Criteria**:
- Given the active session has a previous sibling in its group, when destroyed, then focus moves to the previous sibling
- Given no previous sibling but a next sibling, when destroyed, then focus moves to the next sibling
- Given the group is empty after destruction, then focus moves to the first visible tab or first tab in any group
- Given the last session is destroyed, then the on_empty callback fires

### Session Switching

**Acceptance Criteria**:
- Given multiple visible sessions at index N, when switch_adjacent(forward), then session at (N+1) % total is activated
- Given multiple groups, when switch_adjacent_group(forward), then first session of next group is activated
- Given a visible index, when switch_to_visible_index is called, then that session is activated
- Given a session in a collapsed group, when switched to, then deferred shells are spawned first

### Split Pane Management

**Acceptance Criteria**:
- Given a single-pane session, when horizontal split is requested, then a new pane is created below inheriting the CWD
- Given multiple panes, when focused pane is closed, then the widget tree is rebuilt with focus on a remaining pane
- Given one pane remaining, when close is invoked, then the caller is signaled to destroy the session
- Given a pane's child exits and it's the last pane, then the session is destroyed

### Title and CWD Tracking

**Acceptance Criteria**:
- Given a VTE title matching shell prompt pattern, when title changes, then tab shows the folder name
- Given a VTE title with a running command, when title changes, then tab shows the command name
- Given a CWD change, when detected, then folder/subtitle update and git branch + PR detection triggers
- Given a CWD change to the same directory, then the update is skipped

### Non-Claude Status Tracking

**Acceptance Criteria**:
- Given a non-Claude session with a running command, when 3 seconds elapse, then a "Running" pill appears
- Given the command finishes before 3 seconds, then no pill is shown
- Given the Running pill was shown and the command finishes in a background tab, then the pill resets to Idle and a notification is emitted
- Given the command finishes in the active tab, then no notification is emitted

### Bell Notifications

**Acceptance Criteria**:
- Given a bell in a background session with no bell in the last 2 seconds, then a notification is added
- Given a bell within 2 seconds of the previous bell for the same session, then it is discarded
- Given a bell in the active session, then no notification is emitted

### Notification and Status Navigation

**Acceptance Criteria**:
- Given sessions with unread notifications, when switch_adjacent_with_notifications is called, then the next session with unreads is activated
- Given sessions with Running status or a running command detected via VTE title heuristics, when switch_adjacent_running is called, then the next matching session is activated

### Session Persistence

**Acceptance Criteria**:
- Given sessions with splits/groups/Claude IDs, when save_state is called, then a complete SessionState is produced
- Given saved state, when restore is called, then sessions are recreated with split trees, CWDs, and pending resume IDs
- Given collapsed groups, when spawn_deferred is called, then only non-collapsed group sessions have shells spawned

### Claude Integration

**Acceptance Criteria**:
- Given restored sessions with Claude session IDs, when take_pending_resumes is called, then (session_id, claude_session_id) pairs are returned for non-collapsed groups
- Given a specific group, when take_pending_resumes_for_group is called, then pairs for that group are returned

### Environment Variables

**Acceptance Criteria**:
- Given any session spawning a shell, then SEEMUX_SOCKET and SEEMUX_SESSION_ID are set
- Given agent_teams_shim enabled, then the bin directory is prepended to PATH
