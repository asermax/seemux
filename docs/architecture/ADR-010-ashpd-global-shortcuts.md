# ADR-010: ashpd for XDG Portal Global Shortcuts

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

The quake-mode dropdown needs a global keyboard shortcut that works regardless of which application is focused. On Wayland, applications cannot grab global keys directly — this must go through the compositor via the XDG Desktop Portal GlobalShortcuts API.

## Decision

Use the `ashpd` crate for async XDG Desktop Portal bindings. Register a `toggle-dropdown` shortcut at startup. Activations are received via a stream and forwarded to `dropdown.toggle()`. Gracefully degrade if the portal is unavailable.

## Consequences

### Positive

- Standard Wayland mechanism for global shortcuts
- Works across compositors that implement the portal
- Graceful fallback — CLI toggle and tray click remain available

### Negative

- Requires a running XDG Desktop Portal instance
- Not all compositors implement GlobalShortcuts (e.g., older Sway versions)
- The shortcut binding is suggested, not guaranteed — the compositor may override it

## Alternatives Considered

### Compositor-specific protocol (e.g., Hyprland binds)

- **Description**: Use compositor-specific keybinding APIs
- **Why rejected**: Not portable across compositors

### D-Bus GlobalShortcuts directly

- **Description**: Raw D-Bus calls without ashpd
- **Why rejected**: ashpd provides a higher-level async API with type safety

### X11 key grabbing

- **Description**: Use XGrabKey for global shortcuts
- **Why rejected**: Does not work on Wayland
