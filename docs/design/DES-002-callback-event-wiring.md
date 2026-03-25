# DES-002: Callback-Based Event Wiring

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

Components expose `set_on_*` methods accepting boxed closures stored in `Rc<RefCell<Option<Box<dyn Fn(...)>>>>`. The closure is called when the component's internal state changes, allowing the parent to react without the component knowing about the parent.

## Rationale

Decouples components without introducing a message bus or trait-based observer. The closure captures `Rc` references to other components (or `Weak` references to avoid cycles), enabling cross-component coordination within GTK's single-threaded model.

## Examples

### Do This

```rust
// Component stores callback as Rc<RefCell<Option<Box<dyn Fn(args)>>>>
// Parent sets callback after construction via set_on_change(closure)
// Component calls callback when state mutates
```

**Why**: Simple, type-safe, no overhead. The parent decides what to do; the component just notifies.

### Don't Do This

```rust
// Component holds direct reference to parent and calls parent methods
// Or: Global event bus with string-keyed events
```

**Why**: Direct references create coupling. Global buses lose type safety and add runtime overhead.

## Exceptions

- GTK signals (`connect_*`) are used for GTK widget events; `set_on_*` is for domain-level events between seemux components.

---

## Related

- Used by: `Sidebar`, `NotificationStore`, `StatePersistence`, `TabGroupWidget`, `CollapsedBar`
