# Application Shell

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/main.rs`, `src/cli.rs`, `src/app/mod.rs`, `src/app/actions.rs`, `src/app/dialogs.rs`, `src/app/keyboard.rs`.
Retrofit date: 2026-03-24

---

## Overview

The application shell is the outermost layer of seemux. It owns the GTK application lifecycle, CLI argument parsing, window construction for two modes (normal windowed and quake/dropdown), subsystem wiring, GIO action registration, overlay dialogs, keyboard shortcut handling, and signal-safe state persistence. It bootstraps and connects all subsystems but delegates domain logic to dedicated modules.

## User Stories

- As a developer using Claude Code across multiple projects, I want a tabbed terminal multiplexer with keyboard-driven navigation, split panes, session groups, and dropdown mode.
- As a developer, I want a dropdown terminal I can summon and dismiss with a global shortcut.
- As a user, I want to toggle the dropdown from the CLI (`seemux toggle`) so I can bind it to a system-wide hotkey.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | GTK4 terminal multiplexer with two window modes (normal and quake), keyboard-driven navigation, split panes, session groups, and persistence |
| R1 | CLI: normal (no args), quake (`--quake`), or command (`toggle` via socket) |
| R2 | Normal mode: windowed app with resizable sidebar-paned layout, overlay dialogs, quit-on-last-tab-closed |
| R3 | Quake mode: dropdown with auto-hide, dialog-mode detection, global shortcut, respawn tab on last close |
| R4 | GIO actions for terminal ops: copy, paste, split-h, split-v, close pane/tab, open-url, edit-file |
| R5 | Right-click context menu adapts to content under cursor (URL, file, plain text) |
| R6 | Ctrl+Click on URLs opens them (text files in editor, others in browser) |
| R7 | Overlay dialogs: new group, rename group, confirm group deletion; dismissible via Cancel/Escape |
| R8 | Keyboard shortcuts in capture phase: Ctrl+Shift (copy/paste/new/close/split/sidebar/group), Alt+hjkl (pane nav), Alt/Ctrl+1-9 (tab index), Ctrl+Tab (cycle), PageUp/Down variants |
| R9 | Alt-hold shows numeric tab index overlays (1-9) in sidebar |
| R10 | Restore persisted state on startup; create fresh tab if no saved state |
| R11 | Deferred shell spawning (idle callback) and Claude resume (500ms delay) after restore |
| R12 | Persist state on window close, SIGTERM, SIGHUP via atomic flag + GTK poll |
| R13 | Sidebar collapse/expand with position memory and drag handle locking |
| R14 | GTK_IM_MODULE workaround for dead keys on Wayland with GTK 4.20+ |
| R15 | Notification integration: badge/preview updates, tab peek, tray count, collapsed-bar dot clicks |
| R16 | DnD tab movement callbacks from sidebar to session manager |
| R17 | Quake dialog mode via Wayland toplevel monitoring |
| R18 | Generation-counted delayed hide (300ms) and spurious-focus-loss recovery (150ms) |
| R19 | Edit-file reuses existing neovim session per parent terminal via nvim --remote |
| R20 | New tabs inherit active terminal's CWD and group |
| R21 | Tab/group actions as GIO actions with string params; group delete shows confirmation if non-empty |
| R22 | hide-dropdown action in quake mode for URL opening |
| R23 | Hook polling (100ms) dispatches events, commands, and application-level controls |
| R24 | Stale PID detection every 5 seconds |
| R25 | Optional system tray icon with unread count updates |

## Behaviors

### CLI Argument Parsing

**Acceptance Criteria**:
- Given no arguments, then normal window mode
- Given `--quake`, then quake/dropdown mode
- Given `toggle`, then toggle-dropdown message sent to socket and process exits
- Given `toggle` with no running instance, then error printed and process exits

### Normal Window

**Acceptance Criteria**:
- Given normal mode, when activated, then 1000x700 window with sidebar-paned layout and overlay
- Given last tab closed, then application quits
- Given window close, then state persisted, tray shut down, app quits

### Quake Window

**Acceptance Criteria**:
- Given quake mode, when activated, then dropdown presented hidden with global shortcut registered
- Given all tabs closed in quake mode, then a new session is created instead of quitting
- Given focus loss, then 300ms grace period before hide (unless dialog mode or focus regained)

### GIO Actions

**Acceptance Criteria**:
- Given selected text, when term-copy, then selection copied to clipboard
- Given active terminal, when term-paste, then clipboard pasted
- Given active session, when split-h/split-v, then new pane created
- Given single pane, when term-close, then session destroyed; multiple panes, only pane closed
- Given a URL, when open-url, then opened in default handler
- Given a text file URI and no existing editor, when edit-file, then new nvim session created
- Given a text file URI and existing editor with live socket, when edit-file, then nvim --remote to existing instance

### Context Menu

**Acceptance Criteria**:
- Given right-click over plain text, then Copy/Paste/Split-H/Split-V/Close
- Given right-click over HTTP URL, then additionally "Open URL"
- Given right-click over file:// text file, then "Open in Editor" and "Open with external App"
- Given right-click over file:// non-text, then "Open with external App" only

### Ctrl+Click

**Acceptance Criteria**:
- Given Ctrl+click on a file:// text URL, then edit-file triggered
- Given Ctrl+click on an HTTP URL, then open-url triggered
- Given Ctrl+click on non-URL area, then passed through to terminal

### Overlay Dialogs

**Acceptance Criteria**:
- Given new-group triggered, then entry form with placeholder, Cancel/Create buttons
- Given Cancel or Escape, then overlay dismissed and terminal focus restored
- Given non-empty entry and Create/Enter, then group created with first tab
- Given rename-group triggered, then entry prefilled with current name, text selected
- Given group delete with tabs, then confirmation dialog shown

### Keyboard Shortcuts

**Acceptance Criteria**:
- Ctrl+Shift+C/V: copy/paste
- Ctrl+Shift+T: new tab in same group inheriting CWD
- Ctrl+Shift+W: close pane/session
- Ctrl+Shift+H/E: split horizontal/vertical
- Alt+h/j/k/l: pane navigation (left/down/up/right)
- Alt+N / Ctrl+N (1-9): switch to visible tab index
- Ctrl+Tab / Ctrl+Shift+Tab: next/previous tab
- Alt+PageDown/Up: adjacent tab
- Alt+Shift+PageDown/Up: next/previous tab with notifications
- Ctrl+Alt+PageDown/Up: next/previous group
- Ctrl+Shift+PageDown/Up: next/previous running session
- Ctrl+Shift+B: toggle sidebar collapse
- Ctrl+Shift+.: toggle active group collapse
- Ctrl+Shift+G: new group dialog

### Session Restoration

**Acceptance Criteria**:
- Given persisted state, when started, then groups/sessions/splits/active restored
- Given no persisted state, then fresh tab created
- Given restored sessions, when idle callback fires, then shells spawn for non-collapsed groups
- Given sessions with Claude IDs, when 500ms after spawn, then resume commands fed
- Given collapsed group expanded, then deferred spawn + resume for that group

### Signal Handling

**Acceptance Criteria**:
- Given SIGTERM/SIGHUP, then atomic flag set, next 100ms poll persists state and quits
