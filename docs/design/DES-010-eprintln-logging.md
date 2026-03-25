# DES-010: Error Logging via eprintln

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

All error and diagnostic output uses `eprintln!` to stderr. No log crate, no log levels, no structured logging.

## Rationale

Seemux is a desktop application, not a server. Errors are rare and typically indicate configuration issues or external tool failures. `eprintln!` is zero-dependency and sufficient for the current needs. Users can redirect stderr to a file for debugging.

## Examples

### Do This

```rust
// eprintln!("Failed to load config: {err}");
// eprintln!("Hook server: invalid JSON from client");
```

**Why**: Zero dependency, simple, consistent.

### Don't Do This

```rust
// Silently ignore errors
// Or: panic on recoverable errors
```

**Why**: Silent errors make debugging impossible. Panics crash the application.

## Exceptions

- If structured log analysis becomes needed (e.g., for automated testing or telemetry), consider adopting the `log` crate with `env_logger` or `tracing`.

---

## Related

- Used by: All modules
