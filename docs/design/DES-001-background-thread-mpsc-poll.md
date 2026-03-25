# DES-001: Background Thread + mpsc + GTK Poll

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

When blocking work must run off the GTK main thread, spawn a `std::thread`, send results through `std::sync::mpsc::channel`, and poll `try_recv()` via a `glib::timeout_add_local` timer on the GTK main thread.

## Rationale

GTK4 is single-threaded — UI mutations cannot happen from background threads. The mpsc + poll pattern bridges this gap simply, without requiring an async runtime (tokio/async-std) or GLib-specific channel types.

## Examples

### Do This

Spawn a background thread, poll results at a fixed interval, and self-terminate when done:

```rust
// Background thread does blocking work, sends result
// GTK timer polls try_recv() every N ms
// Timer returns Break on result or channel disconnect
```

**Why**: Simple, self-cleaning, no async runtime. Each pending operation uses one GTK timer.

### Don't Do This

```rust
// Blocking work directly on the GTK main thread
// Or: glib::idle_add from a background thread with Send bounds
```

**Why**: Blocking the main thread freezes the UI. `idle_add` from background threads requires `Send` bounds incompatible with GTK widget types.

## Exceptions

- For one-shot work with no result needed, `std::thread::spawn` with no channel is sufficient.
- If GLib channel bindings improve, `glib::MainContext::channel()` could replace polling.

---

## Related

- Used by: `notifications/hook_server.rs` (100ms), `toplevel_monitor.rs` (100ms), `git.rs` (50ms)
- Related feature: [../feature-designs/hooks/claude-code-integration.md](../feature-designs/hooks/claude-code-integration.md)
