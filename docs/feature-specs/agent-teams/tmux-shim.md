# Tmux Shim -- Agent Teams Compatibility

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/bin/seemux_tmux_shim.rs`, `src/runtime.rs`.
Retrofit date: 2026-03-24

---

## Overview

Claude Code Agent Teams uses tmux as its multiplexer backend. The tmux shim is a standalone binary that masquerades as `tmux` on the PATH. When running inside a seemux terminal (detected via `$SEEMUX_SOCKET`), it translates tmux subcommands into seemux socket protocol commands. When outside seemux, it transparently delegates to `/usr/bin/tmux`. The shim maintains a file-locked JSON pane map to correlate tmux-style pane IDs with seemux session UUIDs, safe for concurrent access from multiple agent processes.

## User Stories

- As a Claude Code user running Agent Teams inside seemux, I want teammate agents to appear as native seemux sessions in a sidebar group so I can monitor them through the same interface.
- As a user who sometimes runs Claude Code outside seemux, I want the shim to fall through to real tmux transparently.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Intercept tmux commands from Agent Teams and translate them into seemux session/group operations when `$SEEMUX_SOCKET` is set |
| R1 | Fall through to `/usr/bin/tmux` when `$SEEMUX_SOCKET` is not set |
| R2 | Maintain a file-locked pane map (`pane-map.json`) correlating tmux pane IDs to seemux session IDs |
| R3 | On split-window/new-window, allocate a pane ID with `__pending__` placeholder; defer session creation to send-keys |
| R4 | On send-keys with a Claude launch command targeting a pending pane, create a named group ("Team: {team_name}") and session, updating the pane map |
| R5 | On send-keys targeting a resolved pane, forward text as raw input to the seemux session |
| R6 | On kill-pane, remove from map and destroy the seemux session |
| R7 | Report synthetic tmux version (`tmux 3.4`) on `-V` flag |
| R8 | On display-message with `#{pane_id}`, return `%0` and ensure the lead pane entry exists |
| R9 | On display-message with `#{session_name}:#{window_index}`, return `seemux:0` |
| R10 | On list-panes, return all pane IDs from the map sorted numerically |
| R11 | No-op for layout/cosmetic commands: set-option, select-layout, resize-pane, has-session |
| R12 | Log unhandled subcommands to stderr and return success |
| R13 | Provide seemux-env subcommand for toggling `$TMUX` environment variable |
| R14 | Deploy shim binary as `tmux` in runtime bin directory, preferring symlink with copy fallback |
| R15 | Feature gated behind `agent_teams_shim` config flag (default: false); when enabled, runtime bin dir is prepended to PATH |
| R16 | On select-pane with `-T <title>`, stash `(pane_id → title)` in `pending-titles.json` (file-locked); other select-pane forms are no-ops |

## Behaviors

### Activation and Fallthrough

**Acceptance Criteria**:
- Given `$SEEMUX_SOCKET` is set, when any tmux subcommand is invoked, then it is handled via the seemux socket
- Given `$SEEMUX_SOCKET` is not set, when invoked, then arguments are passed to `/usr/bin/tmux`
- Given `$SEEMUX_SOCKET` is set with no arguments, then exit successfully with no output

### Version Reporting

**Acceptance Criteria**:
- Given `-V` in arguments, then print `tmux 3.4` and exit

### Pane Allocation

**Acceptance Criteria**:
- Given the pane map has `%0` and `%1`, when `split-window -P` is invoked, then `%2` is added as `__pending__` and printed to stdout
- Given `split-window` without `-P`, then pane is allocated but nothing is printed

### Teammate Session Creation

**Acceptance Criteria**:
- Given pane `%1` is `__pending__`, when `send-keys` has a Claude launch command with `--team-name my-team --agent-name writer`, then a group "Team: my-team" is created, a session titled "writer" is created with the command, and the pane map is updated
- Given a `--agent-name` flag is present, then session title is taken from `--agent-name` (preferred over any stashed `select-pane -T` title)
- Given no `--agent-name` flag but a stashed title from an earlier `select-pane -T` on the same pane, then the stashed title is used
- Given neither `--agent-name` nor a stashed title, then session title defaults to "teammate"
- Given a stashed title exists for the pane, then it is popped from `pending-titles.json` whether or not it was used, so it cannot bleed into a later pane reuse

### Raw Input Forwarding

**Acceptance Criteria**:
- Given pane `%1` maps to a real session ID, when `send-keys` is invoked with text and `Enter`, then send-input is sent with the text plus newline
- Given pane `%1` is `__pending__` with a non-Claude command, then no socket command is sent

### Pane Operations

**Acceptance Criteria**:
- Given `list-panes`, then all pane IDs are returned sorted numerically
- Given `kill-pane -t %2` mapping to a session, then the map entry is removed and destroy-session is sent
- Given `kill-pane -t %2` mapping to `__pending__` or `__lead__`, then only the map entry is removed

### Display Message

**Acceptance Criteria**:
- Given format `#{pane_id}`, then `%0` is returned and the lead pane entry is ensured
- Given format `#{session_name}:#{window_index}`, then `seemux:0` is returned

### Pane Map Concurrency

**Acceptance Criteria**:
- Given multiple concurrent shim processes, when they access the pane map, then exclusive file locking prevents races

### Deployment

**Acceptance Criteria**:
- Given `agent_teams_shim` is true and the shim binary exists, then it is deployed as `$XDG_RUNTIME_DIR/seemux/bin/tmux`
- Given `agent_teams_shim` is true, when a session spawns, then the bin directory is prepended to PATH
- Given `agent_teams_shim` is false, then no shim is deployed and PATH is unmodified
