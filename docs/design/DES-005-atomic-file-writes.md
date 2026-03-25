# DES-005: Atomic File Writes

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

All file persistence uses the tempfile + rename pattern: create a `NamedTempFile` in the same directory as the target, write content, then call `persist()` (which uses POSIX `rename(2)`). This guarantees the file is either fully written or untouched — never partially written.

## Rationale

A crash or power loss during a direct write can leave a truncated or corrupted file. `rename(2)` is atomic on POSIX when source and target are on the same filesystem. Creating the temp file in the same directory guarantees same-filesystem.

## Examples

### Do This

```rust
// Create NamedTempFile in parent directory of target
// Write all content to temp file
// Call persist(target_path) for atomic rename
```

**Why**: Crash-safe. Previous file survives failed write.

### Don't Do This

```rust
// Write directly to target file
// Or: Write to temp file in /tmp then rename (cross-filesystem rename fails)
```

**Why**: Direct write corrupts on crash. Cross-filesystem rename falls back to copy, losing atomicity.

## Exceptions

- For truly ephemeral data (logs, debug output), direct writes are acceptable.

---

## Related

- Used by: `config.rs` (Config::save, SessionState::save)
- Related feature: [../feature-designs/persistence/configuration-and-state.md](../feature-designs/persistence/configuration-and-state.md)
