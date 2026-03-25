# Terminal Emulation and Split Panes

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/terminal/`.
Retrofit date: 2026-03-24

---

## Overview

Seemux embeds a fully configured VTE4 terminal emulator in each pane, supporting shell/command spawning, font rendering, color theming, scrollback, URL detection, and Shift+Enter passthrough (kitty protocol). Panes are organized in a per-session binary split tree supporting horizontal/vertical splits with directional navigation, and the entire layout serializes to JSON for persistence across restarts.

## User Stories

- As a developer, I want a fully configured terminal emulator in each pane so that I can run shells and commands with proper rendering without additional setup.
- As a developer, I want to split panes horizontally or vertically and navigate between them with directional keys so that I can view multiple processes side by side.
- As a developer, I want my scroll position to remain stable during TUI re-renders so that I don't lose my place in output.
- As a developer, I want my split pane layouts and per-pane working directories restored on restart so that I can resume where I left off.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Provide an embedded VTE4 terminal widget that spawns shells and commands with user-configurable font, scrollback, and color scheme |
| R1 | Detect and highlight URLs via both OSC 8 hyperlinks and regex matching, with smart resolution of relative paths against the terminal's CWD |
| R2 | Pass Shift+Enter to the child process using the kitty keyboard protocol escape sequence (`\x1b[13;2u`) |
| R3 | Scroll guard preserves the user's scroll offset when VTE internally mutates the scroll adjustment, restoring position automatically |
| R4 | Manage an arbitrary binary tree of split panes per session, supporting horizontal and vertical splits at any leaf |
| R5 | Directional navigation (left, right, up, down) across the split tree, moving focus to the nearest neighbor pane |
| R6 | Closing a pane promotes its sibling; closing the last pane destroys the session |
| R7 | Rebuild the GTK widget tree from the split data structure on demand, properly tearing down old Paned hierarchies |
| R8 | Serialize the split tree to JSON (preserving orientation and per-pane CWDs) and deserialize to restore layouts |
| R9 | Inherit and allow overriding of environment variables when spawning child processes |

## Behaviors

### Terminal Creation and Configuration

**Acceptance Criteria**:
- Given a Config with font, scrollback, and color scheme settings, when a new VteTerminal is created, then it is configured with the matching Pango font, scrollback lines, scroll-on-keystroke enabled, audible bell disabled, bold-is-bright enabled, and scheme colors applied
- Given a newly created VteTerminal, when the widget is retrieved, then it is a horizontal Box containing the VTE terminal (hexpand/vexpand) followed by a vertical scrollbar

### Shell and Command Spawning

**Acceptance Criteria**:
- Given an unspawned VteTerminal, when `spawn_shell` is called with no CWD, then the user's `$SHELL` is spawned (falling back to `/bin/bash`)
- Given an unspawned VteTerminal, when `spawn_shell` is called with a CWD, then the shell starts in that directory
- Given a VteTerminal, when `spawn_command` is called with extra env vars, then the command is spawned with the full inherited environment plus the extra vars (replacing, not duplicating)

### Shift+Enter Passthrough

**Acceptance Criteria**:
- Given a focused VTE terminal, when the user presses Shift+Enter, then `\x1b[13;2u` is fed to the child process and the event does not propagate
- Given a focused VTE terminal, when Enter is pressed without Shift, then the event propagates normally

### URL Detection and Resolution

**Acceptance Criteria**:
- Given terminal output with an OSC 8 hyperlink, when `check_url_at` is called at those coordinates, then the OSC 8 URL is returned (priority over regex)
- Given output containing "https://example.com/path", when `check_url_at` is called, then the full URL is returned
- Given output containing "www.example.com", when `check_url_at` is called, then "https://www.example.com" is returned
- Given output containing "./relative/path" and a known CWD, when `check_url_at` is called, then the path is resolved against the CWD
- Given output containing a relative path and no known CWD, when `check_url_at` is called, then the raw string is returned

### Scroll Guard

**Acceptance Criteria**:
- Given the user has scrolled up, when VTE internally changes the scroll adjustment, then the guard restores the original distance from bottom
- Given the user is at the bottom, when VTE changes the adjustment, then no restoration occurs
- Given the user scrolls via mouse wheel, keyboard, or scrollbar, when value_changed fires, then the interaction is recognized as user-initiated and the offset is updated
- Given the user scrolls back to the bottom, when within 1 row of maximum, then the guard is disabled until the user scrolls up again
- Given VTE switches to alternate screen with bounds too small for the stored offset, when restoration is attempted, then it is deferred

### Pane Splitting

**Acceptance Criteria**:
- Given a single-pane session, when a horizontal split is requested, then the tree becomes `Split(H, Leaf(A), Leaf(B))` with focus on the new pane
- Given a split session with pane B focused, when a vertical split is requested, then pane B's leaf becomes `Split(V, Leaf(B), Leaf(C))` with focus on C
- Given any split operation, when it completes, then a new UUID pane ID is assigned and the caller must explicitly rebuild the GTK widgets

### Directional Navigation

**Acceptance Criteria**:
- Given `Split(H, A, B)` with A focused, when navigating Right, then focus moves to B
- Given the same layout with A focused, when navigating Left, then None is returned (already leftmost)
- Given a perpendicular navigation direction, when no neighbor exists in that direction, then None is returned

### Pane Closing

**Acceptance Criteria**:
- Given multiple panes with B focused in `Split(H, A, B)`, when B is closed, then A is promoted and focus moves to A
- Given a single pane, when it is closed, then the session should be destroyed

### Split Tree Serialization

**Acceptance Criteria**:
- Given a split tree with per-pane CWDs, when `to_saved` is called, then a `SavedSplitNode` tree is produced preserving orientation and CWDs
- Given a `SavedSplitNode`, when `from_saved` is called, then a new SplitView is constructed with fresh UUIDs, configured VteTerminals, and a list of (pane_id, optional_cwd) pairs for shell spawning
