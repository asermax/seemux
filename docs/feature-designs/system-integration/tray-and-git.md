# Design: System Tray and Git/PR Detection

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/system-integration/tray-and-git.md](../../feature-specs/system-integration/tray-and-git.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/tray.rs`, `src/git.rs`.
Retrofit date: 2026-03-24
Decisions discovered: Tray-to-app socket reuse (ADR), Bitmap font for badges (DES), Generic async helper with polling (DES)

---

## Purpose

This document explains the design rationale for system tray integration and git/PR detection: the SNI protocol adapter, bitmap badge rendering, cross-thread communication, and the async-to-main-thread delivery pattern.

## Problem Context

Seemux may be minimized or in quake mode. Without a tray, users have no ambient status indicator. Without git indicators, developers must manually check branches. GTK4 is single-threaded; tray runs on its own ksni thread; git/gh commands are blocking I/O.

## Design Overview

Two independent subsystems sharing a common async delivery pattern:

**System Tray** — `SeemuxTray` implements `ksni::Tray` on its own thread. GTK main thread updates via `ksni::Handle`. Tray communicates back by writing JSON to the existing hook Unix socket.

**Git/PR Detection** — Generic `run_async` helper spawns background thread, polls result via 50ms GTK timer. `detect_branch_async` chains into `detect_pr_async`.

## Components

| Component | Responsibility |
|-----------|----------------|
| SeemuxTray | SNI protocol, icon rendering, badge overlay, click dispatch |
| TrayHandle | Thread-safe update bridge (Rc\<RefCell\<Option\<ksni::Handle\>\>\>) |
| DIGIT_GLYPHS | 4x6 bitmap font for badge digits 0-9 and "+" |
| run_async | Generic background-thread-to-GTK-main-thread delivery |
| detect_branch_async | Git branch detection via `git rev-parse` |
| detect_pr_async | GitHub PR detection via `gh pr list` |

## Data Flow

### Badge Update

Notification change → `on_change` callback → `tray.update_count(total)` → `ksni::Handle.update()` on ksni thread → if count changed: `render_badge_icons` at 4 sizes → D-Bus SNI protocol update.

### Tray Click

User clicks → `SeemuxTray::activate()` on ksni thread → JSON to Unix socket → HookServer → mpsc → 100ms poll → `window.present()` or `dropdown.toggle()`.

### Git/PR Chain

CWD change → `detect_branch_async` → background `git rev-parse` → 50ms poll → callback → if branch found: `detect_pr_async` → background `gh pr list` → 50ms poll → callback → sidebar tab row updated.

## Key Decisions

### Tray-to-App via Socket Reuse

**Choice**: Write JSON to the hook Unix socket rather than a dedicated channel.
**Why**: ksni thread has no GTK access. Existing HookServer already handles socket → main thread dispatch.
**Consequences**: Tray events share hook event namespace. Socket unavailability means silent click failure.

### Custom Bitmap Font for Badges

**Choice**: Hand-coded 4x6 pixel glyphs rendered directly into ARGB buffers.
**Why**: No font rendering library needed. Pixel-perfect at tray icon sizes. Zero runtime dependencies.
**Consequences**: Limited to digits 0-9 and "+". Counts above 9 capped as "9+".

### Box-Filter Downscaling

**Choice**: 48px PNG downscaled to 32px and 22px via area-averaging box filter.
**Why**: Simplest correct downscale. Quality adequate for small icons. No image processing crate needed.

### Generic Async Helper with Polling

**Choice**: `std::thread` + `mpsc::channel` + 50ms `glib::timeout_add_local` polling.
**Why**: No async runtime needed. Simple, self-cleaning pattern. 50ms latency negligible for UI indicators.
**Consequences**: Each pending operation uses one GTK timer slot. Thread panic → channel disconnect → timer self-removes.

### PR Detection Chained After Branch

**Choice**: Sequential — PR lookup called inside branch callback.
**Why**: PR lookup requires branch name as input.
**Consequences**: Two sequential thread spawns per detection. If `gh` is slow, only PR indicator delayed.

## System Behavior

### Not a Git Repository

`git rev-parse` fails → `None` delivered → branch indicator cleared → PR detection skipped → PR cleared.

### gh Not Installed

Branch found but `Command::new("gh")` fails → `.ok()` → `None` → branch shown, PR absent. No error visible.

### Badge Caching

`update_count(5)` when current count is 5 → early return, no icon recomputation.

---

## Notes

- Badge icons cached per count value; recomputed only on change.
- `send_event` constructs JSON manually via `format!` rather than serde — works for simple events but fragile for complex payloads.
- The `run_async` pattern is private to `git.rs` but general enough to extract into a shared utility.
