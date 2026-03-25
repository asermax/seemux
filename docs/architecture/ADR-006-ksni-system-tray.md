# ADR-006: ksni for System Tray (SNI Protocol)

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

Seemux needs a system tray icon for desktop presence, notification badge display, and window activation. Linux desktops use the StatusNotifierItem (SNI) protocol for system trays.

## Decision

Use the `ksni` crate for SNI protocol integration. The tray runs on its own thread managed by ksni. Communication back to the app uses the existing Unix socket (ADR-003). Badge rendering uses a custom 4x6 bitmap font to avoid font rendering dependencies.

## Consequences

### Positive

- Standard SNI protocol works across KDE, GNOME, and other desktops with SNI support
- Self-contained badge rendering (no Cairo/Pango dependency from the tray thread)
- Reuses existing socket for back-communication

### Negative

- ksni thread cannot access GTK objects directly
- Custom bitmap font limited to digits 0-9 and "+"
- Badge icons must be pre-rendered as ARGB pixel data

## Alternatives Considered

### libappindicator

- **Description**: Ubuntu's app indicator library
- **Why rejected**: Deprecated, Ubuntu moved to SNI

### GTK StatusIcon

- **Description**: GTK's built-in status icon
- **Why rejected**: Removed in GTK4
