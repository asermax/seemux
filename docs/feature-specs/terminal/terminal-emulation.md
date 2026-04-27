# Terminal Emulation and Split Panes

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/terminal/`.
Retrofit date: 2026-03-24

---

## Overview

Seemux embeds a fully configured VTE4 terminal emulator in each pane, supporting shell/command spawning, font rendering, color theming, scrollback, URL detection, Shift+Enter passthrough (kitty protocol), and PRIMARY-selection middle-click copy/paste (Linux convention, independent of CLIPBOARD). Panes are organized in a per-session binary split tree supporting horizontal/vertical splits with directional navigation, and the entire layout serializes to JSON for persistence across restarts.

## User Stories

- As a developer, I want a fully configured terminal emulator in each pane so that I can run shells and commands with proper rendering without additional setup.
- As a developer, I want to split panes horizontally or vertically and navigate between them with directional keys so that I can view multiple processes side by side.
- As a developer, I want my split pane layouts and per-pane working directories restored on restart so that I can resume where I left off.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Provide an embedded VTE4 terminal widget that spawns shells and commands with user-configurable font, scrollback, and color scheme |
| R1 | Detect and highlight URLs via both OSC 8 hyperlinks and regex matching, with smart resolution of relative paths against the terminal's CWD |
| R2 | Pass Shift+Enter to the child process using the kitty keyboard protocol escape sequence (`\x1b[13;2u`) |
| R3 | Auto-copy on-screen selection to the PRIMARY selection buffer and paste from PRIMARY on middle-click, independent of the CLIPBOARD pathway |
| R4 | Manage an arbitrary binary tree of split panes per session, supporting horizontal and vertical splits at any leaf |
| R5 | Directional navigation (left, right, up, down) across the split tree, moving focus to the nearest neighbor pane |
| R6 | Closing a pane promotes its sibling; closing the last pane destroys the session |
| R7 | Rebuild the GTK widget tree from the split data structure on demand, properly tearing down old Paned hierarchies |
| R8 | Serialize the split tree to JSON (preserving orientation and per-pane CWDs) and deserialize to restore layouts |
| R9 | Inherit and allow overriding of environment variables when spawning child processes |

## Behaviors

### Terminal Creation and Configuration

**Acceptance Criteria**:
- Given a Config with font, scrollback, and color scheme settings, when a new VteTerminal is created, then it is configured with the matching Pango font, scrollback lines, scroll-on-keystroke enabled, scroll-on-output disabled, audible bell disabled, bold-is-bright enabled, and scheme colors applied
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
- Given a URL soft-wrapped across two or more visual rows, when `check_url_at` is called at any pixel covering the URL (including pixels on rows other than the first), then the full reconstructed URL is returned
- Given a URL on row N followed by a hard newline and unrelated text on row N+1, when `check_url_at` is called over the row-N URL, then only the row-N URL is returned (reconstruction never crosses a hard newline)
- Given two URLs separated by whitespace on the same logical line, when `check_url_at` is called over one of them, then only the URL whose `[start, end)` byte interval contains the click offset is returned

### PRIMARY Selection and Middle-Click Paste

**Acceptance Criteria**:
- Given a focused VTE terminal, when the user selects text (via mouse drag or keyboard selection), then the selected text is written to the PRIMARY selection buffer
- Given the PRIMARY selection buffer contains text, when the user middle-clicks (button 2) anywhere over a terminal's text area, then the PRIMARY contents are fed to the child process via the PTY at the shell's current input position (regardless of the click's x/y in the pane)
- Given the user copies text "X" to CLIPBOARD via Ctrl+Shift+C and then selects different text "Y" in any terminal, when the user middle-clicks, then "Y" (PRIMARY) is pasted; when the user invokes Ctrl+Shift+V or right-click Paste, then "X" (CLIPBOARD) is pasted — the two buffers remain independent
- Given any terminal — initial tab, split pane, dropdown-mode terminal, or restored session — when text is selected and middle-click happens between any pair, then PRIMARY copy and middle-click paste behave uniformly without per-call-site wiring
- Given PRIMARY is empty (nothing selected this session, or the compositor lacks PRIMARY support), when the user middle-clicks in a terminal, then no input is fed to the child process and seemux logs no error

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
