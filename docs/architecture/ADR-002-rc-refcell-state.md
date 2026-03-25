# ADR-002: Rc\<RefCell\<T\>\> for Shared State

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

GTK4 runs a single-threaded event loop. All UI mutations must happen on the main thread. Multiple subsystems (SessionManager, Sidebar, NotificationStore, StatePersistence) need shared mutable access to state, and GTK signal handlers capture references via closures.

## Decision

Use `Rc<RefCell<T>>` for all shared mutable state. Use `Rc::downgrade()` to create `Weak` references in GTK signal handler closures to avoid reference cycles. Use `Rc<Cell<T>>` for simple Copy types (flags, counters).

## Consequences

### Positive

- No synchronization overhead (no Mutex/Arc)
- Rust borrow checker still enforces single-writer at runtime via RefCell
- Weak references prevent reference cycles between SessionManager ↔ Sidebar ↔ signals
- Consistent pattern across the entire codebase

### Negative

- Runtime borrow panics possible (though none observed in practice)
- Cannot move any UI state to background threads if needed in the future
- Static methods with `&Rc<RefCell<Self>>` pattern required for GTK signal wiring is verbose

## Alternatives Considered

### Arc\<Mutex\<T\>\>

- **Description**: Thread-safe shared mutable state
- **Why rejected**: Unnecessary overhead for single-threaded GTK; Mutex would deadlock on re-entrant access from signal handlers

### GObject subclassing with properties

- **Description**: Use GTK's native GObject property system for state management
- **Why rejected**: Verbose, stringly-typed, and the Rust bindings make subclassing cumbersome
