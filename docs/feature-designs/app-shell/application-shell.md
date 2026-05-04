# Design: Application Shell

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/app-shell/application-shell.md](../../feature-specs/app-shell/application-shell.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/main.rs`, `src/cli.rs`, `src/app/mod.rs`, `src/app/actions.rs`, `src/app/dialogs.rs`, `src/app/keyboard.rs`.
Retrofit date: 2026-03-24
Decisions discovered: Two builders + shared core (DES), Capture-phase keyboard interception (DES), Overlay dialog pattern (DES), Deferred spawning with staged timing (DES), Dropdown focus management (ADR)

---

## Purpose

This document explains the design rationale for the application shell: the bootstrap sequence, dual window modes, action dispatch, keyboard handling, overlay dialogs, and persistence orchestration.

## Problem Context

Seemux needs an outermost layer bootstrapping a GTK4 terminal multiplexer with two fundamentally different window modes sharing the same subsystem wiring. Normal mode quits on last tab close; quake mode is a persistent overlay that survives empty tab states, manages focus-loss auto-hiding, and cooperates with external dialogs via Wayland layer-shell.

## Design Overview

Layered bootstrap with shared core:

1. **Entry point** (`main.rs`) — Env workarounds, CLI dispatch, GTK Application
2. **CLI** (`cli.rs`) — Tri-modal: Normal, Quake, or fire-and-forget socket command
3. **Window builders** (`app/mod.rs`) — `build_window` and `build_quake_window` with shared `setup_common`
4. **Actions** (`app/actions.rs`) — GIO SimpleAction registrations for all operations
5. **Dialogs** (`app/dialogs.rs`) — GTK Overlay-based modals
6. **Keyboard** (`app/keyboard.rs`) — Capture-phase key controller

The two modes share ~80% of wiring through `setup_common`. Mode-specific differences in their respective builders.

## Components

| Layer/Component | Responsibility | Key Decisions |
|-----------------|----------------|---------------|
| main.rs | GTK_IM_MODULE workaround, CLI dispatch, app lifecycle | Thread-local for cross-callback state |
| cli.rs | Tri-modal arg parsing | Socket fire-and-forget for toggle |
| app/mod.rs | Window construction, subsystem wiring, restore, signals | Two builders + shared core |
| app/actions.rs | GIO action dispatch for all operations | String-parameterized actions (DES-004); includes `open-in-browser` and `open-in-browser-split` |
| app/dialogs.rs | Overlay-based modal forms and confirmations | Overlay instead of GTK Dialog (DES-007); includes `show_url_input_overlay` and `show_browser_error_overlay` |
| app/keyboard.rs | Capture-phase shortcut handler | KeyEvent::matches() for layout-independent shortcuts |

## Data Flow

### Bootstrap

`main()` → env workaround → `cli::handle_args()` → if command: send socket message + exit. Otherwise: `Application::builder()` → `connect_startup` creates `AppState` (stored in thread-local) → `connect_activate` builds window.

### Window Construction

Both builders follow: create widgets → wire sidebar collapse → create SessionManager → create Overlay → create StatePersistence → `setup_common` (actions, context menus, badges, DnD, signals) → restore sessions → wire state-change callback (AFTER restore) → setup hook polling → setup keyboard → present.

Quake mode additionally: layer-shell setup, toplevel monitor polling, global shortcut registration, focus-loss handling with generation counters.

### Hook Event Processing

HookServer (background) → mpsc → 100ms poll → special events (toggle/activate/quit) handled first → domain events → `handle_hook_event()` → status/notification updates.

### Persistence

State mutation → `mark_dirty()` → 2s debounce → `flush()`. Safety-net: 30s timer. Shutdown: immediate `save_now()`.

## Key Decisions

### Two Separate Builders with Shared Core

**Choice**: `build_window` and `build_quake_window` as distinct functions, both calling `setup_common`.
**Why**: Modes have fundamentally different behaviors (quit vs respawn, present vs layer-shell). A single parameterized builder would be harder to follow.
**Consequences**: Some duplication; changes affecting both modes must be applied in two places.

### Capture-Phase Keyboard Interception

**Choice**: `EventControllerKey` in capture phase, before VTE consumes events.
**Why**: VTE terminals consume most keys. Application shortcuts must intercept first.
**Consequences**: Single large handler enumerating all shortcuts. Symbol/punctuation shortcuts use `KeyEvent::matches()` for layout-independent detection.

### Overlay-Based Dialogs

**Choice**: GTK Overlay children instead of separate Dialog/Window.
**Why**: GTK4 deprecated Dialog. In quake mode, separate windows trigger auto-hide. Overlays stay within the focus model.
**Consequences**: Manual dismiss + refocus. No built-in modal blocking.

### GIO Actions as Command Dispatch

**Choice**: All operations as window-scoped GIO SimpleActions with string parameters.
**Why**: Unified dispatch for context menus, keyboard shortcuts, and Ctrl+Click. Decouples "what" from "how triggered."

### Deferred Shell Spawning

**Choice**: Idle callback for shell spawning, 500ms delay for Claude resume. Collapsed groups defer until expanded.
**Why**: Synchronous spawning during restore blocks GTK main loop. Delay gives shell time to initialize before resume command.
**Consequences**: Brief empty-pane moment after launch. 500ms may need tuning for slower systems.

### Dropdown Focus Management

**Choice**: Three-layer system — dialog mode detection, spurious-focus-loss recovery (150ms), generation-counted auto-hide (300ms).
**Why**: Wayland fires focus-loss for transient situations (clipboard tools, popovers, dialogs). Immediate hide would break usability.
**Consequences**: Complex state machine with empirical timing constants.

## System Behavior

### Editor Reuse

Given user opened file_a from terminal A creating editor E → user opens file_b from terminal A → `editor_sessions` map finds E, verifies nvim socket exists → `nvim --remote` to existing instance → subtitle updated, focus switched to E.

### Alt-Hold Tab Index Overlay

Alt pressed → `show_tab_indices()` displays 1-9 overlays → Alt released → overlays hidden. Window focus loss also hides overlays (handles Alt+Tab swallowing the release).

### URL Input Modal

`Ctrl+Shift+O` pressed → `show_url_input_overlay()` creates centered card (DES-007) with blank URL entry, Cancel and Open buttons. Enter/Open calls `normalize_url()` (auto-prepend `https://`) and activates `win.open-in-browser` action. Escape/Cancel/empty input dismisses overlay.

### Browser Error Overlay

Carbonyl not found during session/split creation → `show_browser_error_overlay()` displays "Carbonyl Not Found" with install instructions. Browser pane crashes within 2s → `on_browser_error` callback shows error overlay with URL and debug port.

### Browser Context Menu Items

Right-click on detected non-file URL → context menu includes "Open in browser tab" (activates `win.open-in-browser`, creates new session) and "Open in browser split" (activates `win.open-in-browser-split`, creates browser pane in current session). These items do not appear for `file://` URLs (which show "Open in Editor" instead).

---

## Notes

- `editor_sessions` HashMap lives as closure-captured local inside edit-file action — not persisted across restarts (intentional, nvim sockets are ephemeral).
- `auto_execute` parameter on `schedule_claude_resumes` is always `true` at both call sites — the pre-type-for-review path appears to be unused infrastructure.
- GTK_IM_MODULE workaround (`unsafe set_var`) is only safe because it runs before any threads are created.
