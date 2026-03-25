# DES-009: Drag-and-Drop with Shared Dragging State

**Scope**: Module-specific (sidebar)
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This pattern was inferred from existing code. Retrofit date: 2026-03-24

---

## Pattern

Tab and group drag-and-drop use shared `Rc<RefCell<String>>` to track the currently dragged item ID. Drag sources set it, drop targets read it to decide visual indicators, and drag-end clears it. Tab drags use `GString` content type; group drags use `Variant` content type, preventing cross-acceptance.

## Rationale

GTK4's `DropTarget::connect_motion` does not provide the drag content, only position. The shared mutable string bridges this gap, allowing motion handlers to know which item is being dragged. Type-differentiated content types prevent tab drops from interfering with group drops.

## Examples

### Do This

```rust
// Shared state: Rc<RefCell<String>> for dragging_id
// DragSource: set dragging_id on prepare, clear on end
// DropTarget: read dragging_id in connect_motion for visual indicators
// Use different content types (GString vs Variant) for different draggable types
```

**Why**: Motion handlers can identify the source. Type separation prevents cross-interference.

### Don't Do This

```rust
// Try to read drag content in motion handler (not available in GTK4)
// Or: Use a single content type for both tab and group drags
```

**Why**: GTK4 doesn't expose content in motion events. Single content type causes tabs and groups to accept each other's drops.

## Exceptions

- If only one type of draggable item exists, the shared state can be a simple flag rather than an ID string.

---

## Related

- Used by: `sidebar/tab_row.rs`, `sidebar/tab_group.rs`, `sidebar/mod.rs`
- Related feature: [../feature-designs/sidebar/navigation-and-organization.md](../feature-designs/sidebar/navigation-and-organization.md)
