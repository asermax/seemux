# Design: Dropdown / Quake-Mode Terminal

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/dropdown/quake-mode.md](../../feature-specs/dropdown/quake-mode.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/dropdown.rs`, `src/layer_shell.rs`, `src/toplevel_monitor.rs`, `src/global_shortcuts.rs`.
Retrofit date: 2026-03-24
Decisions discovered: Direct FFI to layer-shell (ADR), Separate Wayland connection for toplevels (ADR), Generation-counter cancellation (DES), Timestamp bridge for async event correlation (DES)

---

## Purpose

This document explains the design rationale for the quake-style dropdown terminal: layer shell integration, animation, dialog mode, focus recovery, and the orchestration of four independent components.

## Problem Context

On Wayland, window management (raise, lower, keyboard grab) is compositor-controlled. The dropdown must handle: layer-shell surface management, external dialog coexistence, clipboard utilities stealing focus, and three different toggle mechanisms. Events from GTK focus, toplevel monitoring, and timers arrive asynchronously with no ordering guarantees.

## Design Overview

Four components, each handling one concern:

1. **DropdownWindow** (`dropdown.rs`) — Window lifecycle, animation, dialog mode, toggle API
2. **Layer Shell FFI** (`layer_shell.rs`) — Thin unsafe bindings to `libgtk4-layer-shell`
3. **Toplevel Monitor** (`toplevel_monitor.rs`) — Background Wayland thread watching for external windows
4. **Global Shortcuts** (`global_shortcuts.rs`) — XDG Portal hotkey registration

The **orchestrator** in `app/mod.rs::build_quake_window` wires them together with focus tracking, debounced auto-hide, and recovery heuristics.

## Modeling

```
DropdownWindow
├── visible: RefCell<bool>              (logical visibility)
├── animation_generation: Rc<Cell<u32>> (cancellation counter)
├── last_keypress: Rc<Cell<Option<Instant>>>
├── dialog_mode: Rc<Cell<bool>>
└── target_height, animation_ms

Orchestrator state (closures in build_quake_window)
├── recent_toplevel: Rc<Cell<Option<Instant>>>
├── hide_generation: Rc<Cell<u32>>
└── DropdownWindow clones for handlers
```

Three generation counters for cancellation: `animation_generation` (stale frames), `hide_generation` (stale auto-hide timers), `initial_done` (pre-existing toplevels).

## Data Flow

### Toggle (CLI / shortcut / tray)

Toggle event → `DropdownWindow::toggle()` → if hidden: `show()` → `animate(opening=true)` with ease-out cubic. If visible and not in dialog mode: `animate(opening=false)` → `set_visible(false)` on completion. If in dialog mode: exit dialog mode instead of hiding.

### Dialog Mode Entry

External window appears → toplevel `Added { has_parent }` event via mpsc (100ms poll). On KDE, `has_parent` distinguishes dialogs (`Some(true)`) from regular apps (`Some(false)`); on ext protocol, `has_parent` is `None` (no parent info). KDE regular apps (`Some(false)`) are skipped. For KDE dialogs and ext events: `recent_toplevel` timestamp is recorded, and `enter_dialog_mode()` is called if visible — layer drops to BOTTOM, keyboard to NONE. OR: focus lost + `recent_toplevel` within 500ms → enter dialog mode.

### Focus Recovery

`wl-copy` steals focus → focus-loss handler → no recent toplevel but recent keypress → 150ms re-present timer → if still not active and not dialog mode: `window.present()`.

### Auto-Hide

Focus lost → 300ms debounced hide with generation counter → if generation matches and not dialog mode: hide.

## Key Decisions

### Direct FFI to libgtk4-layer-shell

**Choice**: Raw `unsafe extern "C"` bindings (7 functions) instead of the `gtk4-layer-shell` Rust crate.
**Why**: Version incompatibility between the crate and seemux's gtk4-rs version.
**Consequences**: Unsafe code, no type safety, but total control and no dependency conflicts.

### Separate Wayland Connection for Toplevel Monitoring

**Choice**: Own `wayland_client::Connection` on a dedicated thread with `blocking_dispatch`.
**Why**: GTK4 owns the primary Wayland connection and doesn't expose it for custom protocol handling.
**Consequences**: Clean separation, 100ms polling latency mitigated by timestamp bridge.

### Generation-Counter Cancellation

**Choice**: `Rc<Cell<u32>>` counters incremented on new operations; callbacks check for mismatch.
**Why**: GTK tick callbacks and timeouts can't be explicitly cancelled. Generation counters provide cooperative cancellation.
**Consequences**: Zero-allocation, trivially correct for rapid toggling. Pattern is implicit — readers must understand the idiom.

### Two-Protocol Fallback (ext then KDE)

**Choice**: Try `ext-foreign-toplevel-list-v1` first, fall back to `org_kde_plasma_window_management`.
**Why**: ext is cross-compositor standard (wlroots/Hyprland/Sway); KDE uses its own protocol.
**Consequences**: Broad coverage. KDE path needs SKIP_TASKBAR filtering and different event lifecycle handling.

### Timestamp Bridge for Async Event Correlation

**Choice**: `recent_toplevel` and `last_keypress` timestamps (500ms windows) bridge events from different sources.
**Why**: Race condition between GTK focus, toplevel polling, and clipboard focus theft.
**Consequences**: Handles common races gracefully. Magic numbers (500ms, 150ms, 300ms) are tuned heuristically.

## System Behavior

### Rapid Toggle

Animation in progress → new toggle → generation counter increments → old tick callback sees mismatch and stops → new animation takes over from current position.

### Layer-Shell Unsupported

`is_supported()` returns false → all calls no-op → window behaves as regular GTK window.

---

## Notes

- The 500ms/150ms/300ms timing constants are heuristic and may need tuning per compositor.
- Animation uses GTK `add_tick_callback` with `Instant`-based timing, not GTK's animation framework.
- KDE protocol version capped at 18; may need updating.
