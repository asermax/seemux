# ADR-004: Direct FFI to libgtk4-layer-shell

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

The quake-mode dropdown terminal needs Wayland layer-shell integration to render as an overlay surface with exclusive keyboard grab. The `gtk4-layer-shell` Rust crate exists but has version incompatibility with seemux's gtk4-rs version.

## Decision

Use raw `#[link(name = "gtk4-layer-shell")] unsafe extern "C"` bindings (7 functions) in `layer_shell.rs`, with manual GObject pointer casting via `ToGlibPtr`.

## Consequences

### Positive

- No transitive dependency version conflicts
- Total control over which functions are bound
- Small binding surface (7 functions) keeps risk bounded

### Negative

- Unsafe code with no type safety on C calls
- Must manually track libgtk4-layer-shell API changes
- `window_ptr` helper relies on GTK4-rs internal GObject representation

## Alternatives Considered

### gtk4-layer-shell crate

- **Description**: Safe Rust bindings for the library
- **Why rejected**: Version incompatibility with the gtk4-rs version used by seemux

### Direct Wayland protocol implementation

- **Description**: Implement layer-shell without the helper library
- **Why rejected**: Would require reimplementing all GTK4 surface integration from scratch
