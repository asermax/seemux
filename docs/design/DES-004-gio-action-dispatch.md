# DES-004: GIO Action Dispatch for UI Commands

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

Window-level UI commands (copy, paste, split, close, open URL, etc.) are registered as `gio::SimpleAction` instances with optional string parameters. Context menus, keyboard shortcuts, and programmatic code all activate the same actions, providing a single implementation point.

## Rationale

GIO actions decouple "what to do" from "how it was triggered." A single action handler serves context menus (built from `gio::Menu` models), keyboard shortcuts, Ctrl+Click, and programmatic invocations. String parameters carry context (session ID, URL string) from trigger to handler.

## Examples

### Do This

```rust
// Register: SimpleAction::new("term-copy", None)
// Connect: action.connect_activate(|_, _| { /* do copy */ })
// Menu: menu_item("Copy", "win.term-copy")
// Keyboard: activate_action("term-copy", None)
```

**Why**: Single implementation. Menus, shortcuts, and code all go through the same path.

### Don't Do This

```rust
// Duplicate logic in keyboard handler, context menu builder, and click handler
```

**Why**: Three implementations to maintain. Bug fixes must be applied in three places.

## Exceptions

- Simple one-off operations that will never appear in a menu can use direct function calls.

---

## Related

- Used by: `app/actions.rs`, `app/keyboard.rs`
- Related feature: [../feature-designs/app-shell/application-shell.md](../feature-designs/app-shell/application-shell.md)
