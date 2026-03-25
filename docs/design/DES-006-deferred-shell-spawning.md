# DES-006: Deferred Shell Spawning for Collapsed Groups

**Scope**: Module-specific (session)
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

Terminals in collapsed groups are created without spawning shells. Shells are spawned only when: (1) the GTK stack is realized (idle callback after window construction), or (2) the group is expanded, or (3) the tab is explicitly switched to. This saves resources by not spawning invisible shells.

## Rationale

A user may have many sessions across multiple groups, most collapsed. Spawning shells for all of them at startup wastes memory and CPU. Deferred spawning loads only what's visible.

## Examples

### Do This

```rust
// Create VteTerminal with needs_spawn = true
// On stack realize or group expand: check needs_spawn, then spawn_shell
// On tab switch to collapsed group session: spawn if needed first
```

**Why**: Minimizes resource usage. Shell starts only when needed.

### Don't Do This

```rust
// Spawn all shells during session restoration
// Or: Create terminal widgets lazily (too late for GTK stack registration)
```

**Why**: Spawning all shells at startup is wasteful. Lazy widget creation breaks GTK stack naming.

## Exceptions

- Sessions in non-collapsed groups always spawn immediately at startup.

---

## Related

- Used by: `session/manager.rs` (spawn_deferred, spawn_group_sessions), `app/mod.rs` (schedule_deferred_spawn)
