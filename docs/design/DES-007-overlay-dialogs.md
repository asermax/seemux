# DES-007: Overlay Dialog Pattern

**Scope**: Project-wide
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

Modal dialogs (new group, rename group, confirm delete) are built as GTK `Overlay` children positioned at the center of the content area, rather than separate windows. Each dialog is an imperatively-constructed card with Cancel/Submit buttons, Escape handling, and automatic terminal refocus on dismiss.

## Rationale

GTK4 deprecated `gtk::Dialog`. In quake/dropdown mode, opening a separate window triggers focus-loss auto-hide. Overlays stay within the same window's focus model, avoiding accidental dropdown dismissal.

## Examples

### Do This

```rust
// Build a card widget with entry/buttons
// Add as overlay child with center alignment
// Connect Escape key on entry to dismiss
// On dismiss: overlay.remove_overlay(card), refocus terminal
```

**Why**: No focus loss. Works in both normal and dropdown modes. Visual integration with content area.

### Don't Do This

```rust
// Use gtk::Dialog or gtk::Window for modals
// Or: Use a popover anchored to a specific widget
```

**Why**: Separate windows cause dropdown auto-hide. Popovers are too small for form content.

## Exceptions

- For system-level dialogs (file chooser), a separate window is unavoidable — the dropdown handles this via dialog mode.

---

## Related

- Used by: `app/dialogs.rs` (show_entry_overlay, show_confirm_overlay)
- Related feature: [../feature-designs/app-shell/application-shell.md](../feature-designs/app-shell/application-shell.md)
