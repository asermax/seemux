# Configuration and State Persistence

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/config.rs`, `src/persistence.rs`, `src/app_state.rs`, `src/runtime.rs`.
Retrofit date: 2026-03-24

---

## Overview

The persistence domain manages four concerns: TOML configuration loading/saving with serde defaults, JSON session state persistence (tabs, recursive split trees, named groups, active tab), debounced dirty-flag state saving with a safety-net timer, and XDG-compliant runtime directory management. All file writes use atomic temp-file-plus-rename to prevent corruption.

## User Stories

- As a user, I want my terminal preferences persisted across sessions so the app always launches with my chosen settings.
- As a user, I want my open tabs, split layouts, and group assignments to survive restarts so I can resume where I left off.
- As a user, I want state changes saved automatically in the background without lag or disk thrashing.
- As a user or external process, I want stable runtime paths (socket, shim scripts) at predictable XDG locations.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Application state (tabs, splits, groups, active tab) and user configuration must survive restarts without data loss |
| R1 | Config uses TOML format with serde defaults so partial config files remain valid |
| R2 | Session state uses JSON format and supports recursive split tree serialization |
| R3 | All file writes must be atomic (temp file + rename) to prevent corruption on crash |
| R4 | State saves are debounced (2s) to avoid excessive I/O from rapid mutations |
| R5 | A safety-net timer (30s) flushes dirty state even if debounce is bypassed |
| R6 | Window close and OS signals (SIGTERM, SIGHUP) trigger immediate synchronous save |
| R7 | On first launch with no config file, a default config is written to disk |
| R8 | Deserialization is backward-compatible: missing fields use serde defaults without failing |
| R9 | Runtime directory follows XDG convention (`$XDG_RUNTIME_DIR/seemux/`) with UID-based fallback |
| R10 | When `agent_teams_shim` is enabled, the tmux shim binary is deployed to the runtime `bin/` directory |
| R11 | Sidebar width and collapsed state are persisted to config only when they actually change |
| R12 | Hook and toplevel receivers use take semantics, preventing multiple consumers |

## Behaviors

### Config Loading

**Acceptance Criteria**:
- Given a valid TOML file at `~/.config/seemux/config.toml`, when Config::load() is called, then all fields are populated with serde defaults filling omitted fields
- Given a partial config with only `font_size = 16`, when loaded, then font_size is 16 and all other fields hold defaults
- Given no config file exists, when loaded, then defaults are returned and a default config file is written to disk
- Given a malformed TOML file, when loaded, then an error is logged, defaults are returned, and a fresh config is written

### Config Saving

**Acceptance Criteria**:
- Given a valid Config, when saved, then it is serialized to pretty TOML, written to a temp file, and atomically renamed
- Given the config directory doesn't exist, when saved, then parent directories are created first
- Given a disk write failure, when saving, then an error is logged and the app continues

### Session State Loading

**Acceptance Criteria**:
- Given a valid JSON state file at `~/.local/state/seemux/sessions.json`, when loaded, then sessions, groups, and active index are deserialized
- Given no state file exists, when loaded, then an empty SessionState is returned
- Given a state file from an older version missing new fields, when loaded, then deserialization succeeds with serde defaults
- Given malformed JSON, when loaded, then an error is logged and empty defaults are returned

### Session State Saving

**Acceptance Criteria**:
- Given multiple sessions with splits, groups, and an active tab, when save_state() is called, then the full topology is captured including recursive split trees with per-leaf CWDs
- Given sessions reordered in the sidebar, when saved, then sessions are serialized in sidebar display order
- Given a SessionState ready to save, when saved, then atomic write via temp file + rename is used

### Debounced Persistence

**Acceptance Criteria**:
- Given mark_dirty() is called once, then a 2-second timer is scheduled for flush
- Given mark_dirty() is called multiple times within 2s, then only a single flush occurs (timer resets)
- Given dirty state and 30 seconds elapse, then the safety-net timer flushes
- Given clean state when the safety-net fires, then no disk write occurs
- Given save_now() is called, then any pending debounce timer is cancelled and state flushes immediately
- Given a flush executes, then session state is always written but config only if sidebar width/collapsed changed

### Signal Handling

**Acceptance Criteria**:
- Given a SIGTERM is received, then an atomic flag is set, the GTK poll loop detects it, calls save_now(), and quits
- Given a SIGHUP is received, then the same save-and-quit behavior occurs

### Runtime Directory

**Acceptance Criteria**:
- Given `$XDG_RUNTIME_DIR` is set, when runtime_dir() is called, then `$XDG_RUNTIME_DIR/seemux/` is returned
- Given `$XDG_RUNTIME_DIR` is unset, when runtime_dir() is called, then `/tmp/seemux-<uid>/seemux/` is returned

### Tmux Shim Deployment

**Acceptance Criteria**:
- Given `agent_teams_shim` is true and the shim binary exists, when AppState::new() is called, then the shim is symlinked (or copied as fallback) to the runtime `bin/tmux`
- Given symlink fails, when deploying, then it falls back to file copy
- Given the shim binary doesn't exist, when deploying, then a warning is logged and no file is created
- Given `agent_teams_shim` is false, when AppState::new() is called, then shim deployment is skipped

### AppState Initialization

**Acceptance Criteria**:
- Given app startup, when AppState::new(quake) is called, then config is loaded, hook server is started, and shim is conditionally deployed
- Given quake mode, when AppState::new(true) is called, then the toplevel monitor is also started
- Given a receiver is taken, when take_hook_rx() is called again, then None is returned (single-claim)
