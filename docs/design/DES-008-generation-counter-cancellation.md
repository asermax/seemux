# DES-008: Generation Counter for Animation/Event Cancellation

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

Use `Rc<Cell<u32>>` monotonic counters to cancel stale GTK timers and tick callbacks. When a new operation starts, increment the counter. Callbacks capture the counter value at creation time and check it on each tick — if the captured value differs from the current value, the callback is stale and should stop.

## Rationale

GTK tick callbacks (`add_tick_callback`) and timeouts (`timeout_add_local_once`) cannot be explicitly cancelled. The generation counter provides cooperative cancellation that is simple, lock-free (single-threaded), and handles rapid re-triggering correctly.

## Examples

### Do This

```rust
// let gen = counter.get() + 1;
// counter.set(gen);
// add_tick_callback(move |...| {
//     if counter.get() != gen { return Remove; }
//     // do work
// });
```

**Why**: Zero-allocation cancellation. Trivially correct for rapid toggling.

### Don't Do This

```rust
// Use Rc<Cell<bool>> flag for cancellation
// Or: Try to store and remove SourceId for tick callbacks
```

**Why**: Boolean flag doesn't handle rapid re-triggering (two starts → one cancel leaves the wrong callback alive). SourceId removal doesn't work for tick callbacks.

## Exceptions

- For simple timeouts where only one can be active, storing and cancelling `glib::SourceId` is acceptable.

---

## Related

- Used by: `dropdown.rs` (animation_generation), `app/mod.rs` (hide_generation)
- Related feature: [../feature-designs/dropdown/quake-mode.md](../feature-designs/dropdown/quake-mode.md)
