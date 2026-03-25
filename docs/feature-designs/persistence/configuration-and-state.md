# Design: Configuration and State Persistence

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/persistence/configuration-and-state.md](../../feature-specs/persistence/configuration-and-state.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/config.rs`, `src/persistence.rs`, `src/app_state.rs`, `src/runtime.rs`.
Retrofit date: 2026-03-24
Decisions discovered: Debounced dirty-flag pattern (DES), Atomic file writes (DES), Signal handling via AtomicBool polling (ADR)

---

## Purpose

This document explains the design rationale for persistence and configuration: the separation of config vs. state, debounced write scheduling, atomic writes, signal handling, and runtime directory management.

## Problem Context

Seemux must survive restarts without losing user work. Two categories of data have different mutation frequencies: config (rarely changes) and session state (changes on every tab operation). Both must be crash-safe. The GTK main loop is single-threaded, so persistence must be non-blocking. Rapid state mutations must not thrash the disk.

## Design Overview

Four cooperating components:

1. **Config** (`config.rs`) — TOML user preferences with serde defaults
2. **SessionState** (`config.rs`) — JSON session topology with recursive split trees
3. **StatePersistence** (`persistence.rs`) — Debounced dirty-flag scheduler
4. **Runtime** (`runtime.rs`) — XDG directory management and shim deployment

**AppState** (`app_state.rs`) bootstraps everything: loads config, starts hook server, deploys shim, provides single-claim channel receivers.

## Modeling

```
Config (TOML, ~/.config/seemux/config.toml)
├── font_family, font_size, scrollback_lines
├── sidebar_width, sidebar_collapsed
├── color_scheme
├── dropdown_width/height_percent, dropdown_animation_ms
├── agent_teams_shim
└── tray_enabled, tray_icon

SessionState (JSON, ~/.local/state/seemux/sessions.json)
├── sessions: Vec<SavedSession>
│   ├── title, group_id, claude_session_id
│   └── split_tree: SavedSplitNode (recursive)
├── groups: Vec<SavedGroup> (id, name, collapsed)
└── active_session_index

StatePersistence
├── dirty: Cell<bool>
├── debounce_source: Cell<Option<SourceId>>
├── last_sidebar_width / last_sidebar_collapsed
└── references to manager, config, sidebar, paned
```

## Data Flow

### Startup

Config loaded from TOML (defaults fill missing fields). Session state loaded from JSON. Groups recreated first, then sessions with split trees. State change callback wired AFTER restore to prevent saving during load.

### Runtime Mutations

User actions trigger `mark_dirty()` → 2-second debounce timer scheduled. Each call resets the timer. A 30-second safety-net timer catches any missed mutations. Flush writes session state unconditionally; config only if sidebar width/collapsed changed.

### Shutdown

Window close, SIGTERM, or SIGHUP trigger immediate `save_now()`. Signal handling uses `AtomicBool` flag polled every 100ms from GTK main loop (glib 0.22 doesn't expose `g_unix_signal_add` safely in Rust).

### Atomic Write Path

All writes use `tempfile::NamedTempFile` in the same directory as the target, then `persist()` (POSIX `rename(2)`), guaranteeing atomicity.

## Key Decisions

### Separate Formats for Config vs. State

**Choice**: TOML for config (human-editable), JSON for state (recursive structures).
**Why**: Config needs hand-editability; session state has recursive split trees natural in JSON.
**Consequences**: Two serialization paths, two file locations, flush logic decides which to write.

### Debounced Dirty-Flag with Safety-Net Timer

**Choice**: 2-second debounce + 30-second safety-net, using `Cell` types in single-threaded context.
**Why**: Tab switches happen rapidly; debounce batches them. Safety-net guards edge cases.
**Consequences**: Up to 2 seconds of state can be lost on unclean kill.

### Atomic Writes via Temp File + Rename

**Choice**: `NamedTempFile` in same directory + `persist()`.
**Why**: `rename(2)` is atomic on POSIX when same filesystem. Prevents corruption from crash mid-write.
**Consequences**: Requires `tempfile` crate. Failed write leaves previous file intact.

### Signal Handling via AtomicBool Polling

**Choice**: C-level signal handler sets `AtomicBool`, GTK polls every 100ms.
**Why**: Signal handlers are restricted to async-signal-safe functions. glib 0.22 doesn't expose `g_unix_signal_add` safely.
**Consequences**: Up to 100ms latency between signal and save. Single set of handlers per process.

### Single-Claim Channel Receivers

**Choice**: `RefCell<Option<Receiver>>` with `take()` semantics.
**Why**: `mpsc::Receiver` is single-consumer. Take semantics make ownership transfer explicit.
**Consequences**: Second window cannot receive hook events (acceptable — only one window polls).

## System Behavior

### First Launch

No files exist → Config defaults written to disk, empty SessionState returned, single fresh tab created.

### Rapid Tab Switching

5 switches in 1 second → 5 `mark_dirty()` calls, each resets timer → single flush 2 seconds after last switch.

### SIGTERM

Signal handler sets AtomicBool → 100ms poll detects → `save_now()` → `app.quit()`.

---

## Notes

- `save_now()` always writes session state unconditionally (even if clean) — intentional for shutdown paths.
- Debounce (2s) and safety-net (30s) values are hardcoded constants.
- State path falls back from `XDG_STATE_HOME` to `XDG_DATA_HOME` for older distro compatibility.
