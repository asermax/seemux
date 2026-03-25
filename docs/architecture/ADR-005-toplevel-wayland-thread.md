# ADR-005: Separate Wayland Connection for Toplevel Monitoring

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

The dropdown terminal needs to detect when external windows (dialogs, file pickers) appear to enter "dialog mode." This requires subscribing to `ext-foreign-toplevel-list-v1` or KDE plasma window management protocols. GTK4 owns the primary Wayland connection and does not expose it for custom protocol handling.

## Decision

Run a dedicated background thread with its own `wayland_client::Connection`, subscribing to toplevel protocols and forwarding `Added`/`Closed` events via `mpsc` channel polled every 100ms on the GTK main thread.

## Consequences

### Positive

- Clean separation — no interference with GTK's Wayland event loop
- Works across compositors (wlroots, Hyprland, Sway, KDE Plasma)
- Simple event normalization to `ToplevelEvent::Added`/`Closed`

### Negative

- Second Wayland connection per process
- 100ms polling latency mitigated by timestamp bridge in focus handler
- Must handle two different protocol event models (ext vs. KDE)

## Alternatives Considered

### Hooking into GTK's Wayland connection

- **Description**: Use `gdk_wayland_display_get_wl_display` to access GTK's connection
- **Why rejected**: Fragile, unsupported by GTK4-rs, would require unsafe integration with GTK's dispatch

### D-Bus window list query

- **Description**: Poll D-Bus for window lists
- **Why rejected**: No standard cross-compositor D-Bus API for toplevel windows
