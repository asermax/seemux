# Dropdown / Quake-Mode Terminal

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/dropdown.rs`, `src/layer_shell.rs`, `src/toplevel_monitor.rs`, `src/global_shortcuts.rs`.
Retrofit date: 2026-03-24

---

## Overview

A quake-style dropdown terminal anchored to the top of the screen, rendered as a Wayland layer-shell surface. It slides in/out with an ease-out cubic animation, grabs exclusive keyboard input, and manages visibility in response to focus loss, external dialogs, and clipboard utilities. A background thread monitors Wayland toplevels to detect external dialogs, temporarily lowering the dropdown. Toggle via CLI (`seemux toggle`), system tray, or compositor global shortcut.

## User Stories

- As a developer on Wayland, I want a dropdown terminal that slides in from the top on a hotkey so I can quickly access sessions without managing windows.
- As a user, I want the dropdown to automatically lower when an external dialog appears so I can interact with it.
- As a user, I want the dropdown to recover from spurious focus loss caused by clipboard utilities.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Quake-style dropdown terminal that slides from the top, toggleable on/off |
| R1 | Wayland layer-shell surface anchored to top edge, centered, with exclusive keyboard |
| R2 | Animated show/hide with ease-out cubic curve, configurable duration |
| R3 | Toggle via CLI (`seemux toggle`), system tray click, and XDG Portal global shortcut |
| R4 | Size as percentage of primary monitor (width and height configurable) |
| R5 | Enter dialog mode when external toplevel appears: lower layer, release keyboard |
| R6 | Exit dialog mode when external toplevel closes or dropdown regains focus |
| R7 | Recover from spurious focus loss if a recent keypress was detected (150ms delay) |
| R8 | Auto-hide after 300ms debounced grace period on focus loss |
| R9 | Monitor toplevels via ext-foreign-toplevel-list-v1 or KDE plasma as fallback |
| R10 | Start hidden (off-screen) so first toggle has no creation delay |
| R11 | Gracefully degrade when layer-shell is unsupported or global shortcuts portal unavailable |
| R12 | Filter KDE SKIP_TASKBAR windows from toplevel monitoring |
| R13 | Cancel stale animations via generation counter on rapid toggling |

## Behaviors

### Toggle Visibility

**Acceptance Criteria**:
- Given the dropdown is hidden, when toggled, then it slides down with ease-out cubic animation and the terminal receives focus
- Given the dropdown is visible and not in dialog mode, when toggled, then it slides up and becomes invisible
- Given the dropdown is in dialog mode, when toggled, then dialog mode is exited (raised, keyboard reclaimed) instead of hiding

### Animation

**Acceptance Criteria**:
- Given an animation in progress, when a new animation starts, then the previous one stops (generation mismatch) and the new one takes over
- Given show animation completes, then top margin is 0, opacity is 1.0
- Given hide animation completes, then top margin is negative height, opacity is 0.0, window is not-visible

### Layer Shell

**Acceptance Criteria**:
- Given quake mode on a Wayland compositor with layer-shell support, when the dropdown is created, then it is initialized as a top-layer surface with exclusive keyboard and positioned off-screen
- Given layer-shell is unsupported, then all calls are no-ops

### Dialog Mode

**Acceptance Criteria**:
- Given KDE plasma protocol and a toplevel with a parent (dialog/transient) appears while the dropdown is visible (or focus is lost within 500ms), then dialog mode is entered: layer changes to bottom, keyboard to none
- Given KDE plasma protocol and a toplevel without a parent (regular app) appears, then dialog mode is NOT entered
- Given ext-foreign-toplevel protocol (no parent info available), when a toplevel appears (or focus is lost within 500ms of a toplevel), then dialog mode is entered (existing behavior preserved)
- Given dialog mode is active, when the toplevel closes, then dialog mode exits: layer returns to top, keyboard to exclusive
- Given dialog mode is active, when the dropdown regains focus, then dialog mode exits
- Given dialog mode is already active, when enter_dialog_mode is called again, then it is a no-op

### Toplevel Monitoring

**Acceptance Criteria**:
- Given Wayland with ext-foreign-toplevel-list-v1, when the monitor starts, then a background thread dispatches add/close events after initial roundtrip
- Given only KDE plasma protocol available, then it is used as fallback
- Given neither protocol available, then start() returns None
- Given pre-existing toplevels during initial roundtrip, then they are ignored
- Given KDE windows with SKIP_TASKBAR flag, then they are filtered out
- Given KDE plasma protocol, toplevel events include parent window info to distinguish dialogs from regular apps

### Focus Recovery

**Acceptance Criteria**:
- Given a keypress within 500ms and focus is lost, then after 150ms the window is re-presented if not in dialog mode
- Given dialog mode was entered before the recovery timer fires, then recovery is suppressed

### Auto-Hide

**Acceptance Criteria**:
- Given focus is lost, when 300ms elapse without regaining focus or entering dialog mode, then the dropdown hides
- Given focus is regained before 300ms, then the hide timer is cancelled via generation counter

### Global Shortcut

**Acceptance Criteria**:
- Given the XDG Portal GlobalShortcuts API is available, then toggle-dropdown is registered and activations trigger toggle()
- Given the portal is unavailable, then an error is logged and toggle remains available via CLI and tray

### CLI Toggle

**Acceptance Criteria**:
- Given a running seemux instance, when `seemux toggle` is executed, then a toggle-dropdown event is sent via the Unix socket
- Given no running instance, then an error is printed to stderr
