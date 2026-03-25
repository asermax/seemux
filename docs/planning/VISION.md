# Project Vision: Seemux

**A Linux-native GTK4 terminal multiplexer designed for Claude Code integration.**

## Problem

**Who experiences this:**
- Developers running multiple Claude Code sessions on Linux Wayland desktops
- Power users who need a terminal multiplexer with AI-agent awareness

**Current situation:**
- **tmux/screen**: No graphical sidebar, no Claude Code status integration, no desktop tray, no quake-mode dropdown
- **GNOME Terminal/Konsole**: No split panes within tabs, no Claude lifecycle awareness, no Agent Teams compatibility
- **cmux (macOS)**: Solves this problem for macOS with Swift/AppKit/libghostty, but is not available on Linux

**What's needed:**
A Linux-native terminal multiplexer that organizes Claude Code sessions with visual status indicators, notification badges, named groups, split panes, session persistence, and a quake-style dropdown mode -- all with deep integration into the Claude Code lifecycle.

## Core Workflows

### 1. Multi-Session Claude Code Management

**Trigger**: Developer starts working across multiple projects/tasks
**Steps**:
1. Create terminal sessions, optionally organized into named groups
2. Run Claude Code in each session
3. Monitor session status via sidebar pills (Idle, Running, NeedsInput, Error, Completed)
4. Jump to sessions needing attention via keyboard shortcuts or notification badges
5. Sessions persist across restarts, including Claude resume

**Result**: All Claude Code sessions visible and manageable from a single interface

### 2. Quake-Style Dropdown Access

**Trigger**: Developer needs terminal access while working in another application
**Steps**:
1. Press global shortcut (or system tray click)
2. Dropdown slides in from top of screen
3. Work in terminal
4. Press shortcut again or click away to dismiss

**Result**: Instant terminal access without window management

### 3. Agent Teams Integration

**Trigger**: Claude Code spawns teammate agents
**Steps**:
1. Agent Teams issues tmux commands (intercepted by shim)
2. Teammates appear as native seemux sessions in a sidebar group
3. Monitor all teammates alongside regular sessions
4. Teammates are torn down cleanly when done

**Result**: Agent Teams works transparently within seemux

## Scope

### v1 Requirements

**Terminal Emulation:**
- VTE4-based terminal with configurable font, colors, scrollback
- Split panes (horizontal/vertical) with directional navigation
- URL detection (OSC 8 + regex) with Ctrl+Click and context menu
- Scroll guard preserving position during TUI re-renders
- Shift+Enter passthrough (kitty protocol)

**Session Management:**
- UUID-identified sessions with title, CWD, group, status
- Create, switch, close, close-others operations
- Circular tab switching, index-based switching (1-9), group switching
- Deferred shell spawning for collapsed groups

**Sidebar Navigation:**
- Tab rows with title, folder, git branch, PR link, status pill, badge, preview
- Named collapsible groups with peek behavior
- Drag-and-drop tab and group reordering
- Collapsed dot-bar mode
- Alt-hold index overlays

**Claude Code Integration:**
- Hook plugin capturing 8 lifecycle events
- Real-time session status mapping
- Notification store with unread counts
- Stale PID detection
- Claude session resume on restart
- Socket command API for programmatic control

**Dropdown / Quake Mode:**
- Layer-shell overlay anchored to top edge
- Animated show/hide with ease-out cubic
- Dialog mode (lower for external windows)
- Focus-loss recovery for clipboard utilities
- Global shortcut via XDG Portal
- CLI toggle (`seemux toggle`)

**Persistence:**
- TOML config with serde defaults
- JSON session state with recursive split trees
- Debounced dirty-flag saving with safety-net timer
- Atomic writes (tempfile + rename)
- Signal handling (SIGTERM, SIGHUP)

**Theming:**
- Catppuccin Mocha (default) and Dracula color schemes
- Runtime CSS generation from scheme values
- VTE palette, UI chrome, and status colors

**System Integration:**
- System tray via SNI with notification badge (bitmap font renderer)
- Async git branch and GitHub PR detection
- Agent Teams compatibility via tmux shim binary

### Not Now

- [ ] Custom user-defined color schemes (runtime theme loading)
- [ ] Multi-monitor dropdown targeting
- [ ] X11 support (Wayland-only for layer-shell features)
- [ ] Built-in multiplexer protocol (e.g., seemux-native IPC beyond socket commands)
- [ ] Tab search / fuzzy finder
- [ ] Session sharing / collaboration
- [ ] Plugin system beyond Claude Code hooks

## Technical Context

**Platform:**
- Linux with Wayland compositor (Hyprland, Sway, KDE Plasma)
- Graceful degradation without layer-shell (normal window mode)

**Language/Runtime:**
- Rust 2024 edition
- Single-threaded GTK4 event loop

**Dependencies (key):**
- `gtk4` (v4_12): UI toolkit
- `vte4` (v0_76): Terminal emulator widget
- `ksni`: System tray (SNI protocol)
- `ashpd`: XDG Desktop Portal (global shortcuts)
- `wayland-client` / `wayland-protocols`: Toplevel monitoring
- `libgtk4-layer-shell`: Layer-shell integration (FFI)
- `socat`: Hook event delivery (runtime)

**File Locations:**
- Config: `~/.config/seemux/config.toml`
- Session state: `~/.local/state/seemux/sessions.json`
- Runtime: `$XDG_RUNTIME_DIR/seemux/` (socket, shim bin)

**User Interaction:**
- Keyboard-driven with comprehensive shortcut set
- Mouse: context menus, drag-and-drop, Ctrl+Click URLs
- System tray for ambient notification awareness

## Success Criteria

v1 is successful when:
1. Multiple Claude Code sessions can be monitored and managed from a single interface with real-time status
2. Sessions persist across restarts including split layouts, groups, and Claude resume
3. The quake-mode dropdown provides instant terminal access on Wayland
4. Agent Teams teammates appear as native sessions without configuration
5. The sidebar provides at-a-glance awareness of all session states

## Future Considerations

Ideas for v2 and beyond (not committing to these):
- Custom color scheme loading from user-defined TOML/JSON files
- Multi-monitor support for dropdown placement
- Tab search / fuzzy finder for large session counts
- Session templates / profiles for common workflows
- Structured logging for debugging
- Runtime theme switching without restart

---

**Project name**: "Seemux" - a portmanteau of "see" (visual awareness of sessions) and "mux" (multiplexer). The Linux-native counterpart to cmux (macOS).
