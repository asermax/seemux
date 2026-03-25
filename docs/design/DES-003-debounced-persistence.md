# DES-003: Debounced Persistence with Dirty Flag

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

State mutations set a `Cell<bool>` dirty flag and schedule a debounced save timer (2 seconds). Each mutation resets the timer. A safety-net timer (30 seconds) catches any missed mutations. Explicit `save_now()` bypasses debounce for shutdown paths.

## Rationale

Rapid mutations (tab switching, reordering) would thrash the disk with writes. A 2-second debounce batches them into a single write. The safety-net guards against edge cases where debounce is cancelled without flushing.

## Examples

### Do This

```rust
// mark_dirty(): set flag, cancel pending timer, schedule new 2s timer
// save_if_dirty(): if dirty, flush and clear flag
// save_now(): cancel timer, flush immediately
// Safety-net: 30s periodic timer calls save_if_dirty()
```

**Why**: Batches rapid mutations. Safety-net prevents data loss. Explicit save for shutdown.

### Don't Do This

```rust
// Save on every mutation (excessive I/O)
// Or: Fixed-interval save without dirty flag (writes when clean)
```

**Why**: Per-mutation saves thrash disk during rapid tab switching. Fixed-interval without dirty flag wastes I/O on clean state.

## Exceptions

- For data that must be immediately durable (security credentials), skip debounce and write synchronously.

---

## Related

- Used by: `persistence.rs`
- Related feature: [../feature-designs/persistence/configuration-and-state.md](../feature-designs/persistence/configuration-and-state.md)
