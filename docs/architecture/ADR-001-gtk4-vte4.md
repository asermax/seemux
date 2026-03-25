# ADR-001: GTK4 + VTE4 as UI/Terminal Framework

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

Seemux needs a terminal emulator embedded in a native Linux desktop application with tabbed/split pane UI, sidebar navigation, system tray, and Wayland layer-shell support. The project targets Linux-native Wayland desktops.

## Decision

Use GTK4 (v4_12 features) as the UI toolkit and VTE4 (v0_76) as the terminal emulator widget, with Rust bindings via `gtk4-rs` and `vte4-rs`.

## Consequences

### Positive

- GTK4 is the standard Linux desktop toolkit with excellent Wayland support
- VTE4 is the only mature terminal emulator widget for GTK4, used by GNOME Terminal and others
- Rust bindings are well-maintained and provide type-safe access to GObject APIs
- Layer-shell integration is possible via `libgtk4-layer-shell`
- Rich widget set covers all UI needs (Paned, ListBox, Overlay, DrawingArea, etc.)

### Negative

- Single-threaded event loop constrains all state management to `Rc<RefCell<T>>`
- VTE4 has quirks (scroll adjustment mutation, Paned teardown complexity) requiring workarounds
- GTK4 Rust bindings lag behind C API, requiring occasional FFI

## Alternatives Considered

### libghostty (Swift/AppKit)

- **Description**: The terminal library from Ghostty, used by cmux (macOS)
- **Why rejected**: Not available on Linux; Swift/AppKit are macOS-only

### Alacritty/wezterm libraries

- **Description**: Embed terminal rendering from other Rust terminal emulators
- **Why rejected**: Not designed as embeddable widgets; would require significant integration work

### Pure Wayland (no toolkit)

- **Description**: Direct Wayland client without GTK
- **Why rejected**: Would require implementing all widget rendering, input handling, and accessibility from scratch
