# Draft Design: Sidebar Navigation and Session Organization

## Retrofit Note

Inferred from existing code at:
- `src/sidebar/mod.rs`
- `src/sidebar/tab_row.rs`
- `src/sidebar/tab_group.rs`
- `src/sidebar/collapsed_bar.rs`

Retrofit date: 2026-03-24

---

## Problem Context

Seemux is a terminal multiplexer designed for power users running multiple concurrent Claude Code sessions. Without a navigation surface, managing many sessions becomes untenable -- users lose track of which session is doing what, which needs attention, and where things are organized.

The sidebar solves this by providing a persistent, information-dense navigation panel that communicates session identity, state, and organization at a glance.

- **Constraints**: The sidebar runs within a single-threaded GTK4 event loop, so all state must be safely shared via `Rc<RefCell<T>>` / `Rc<Cell<T>>`. The collapsed dot bar uses Cairo drawing for custom rendering where GTK widgets alone are insufficient. Screen real estate is at a premium in a terminal multiplexer, so the sidebar must support collapse modes.
- **Interactions**: The sidebar is purely a view/navigation layer. It does not own sessions or manage their lifecycle -- it receives updates from the `SessionManager` and fires callbacks when the user interacts (tab click, close, drag-and-drop move, group create/rename/delete). It also integrates with the theme system (`ColorScheme`) for status colors in the dot bar, and with the persistence layer (`Config`) for sidebar width and collapse state.
- **Scope**: Covers the sidebar widget tree, tab rows, tab groups, the collapsed dot bar, drag-and-drop reordering, peek behavior for collapsed groups, context menus, and Alt-key index overlays. Does NOT cover session creation/destruction logic, hook event processing, or terminal rendering.

## Design Overview

The sidebar is composed of four cooperating structs that together form a composite widget:

1. **`Sidebar`** -- The top-level orchestrator. Owns the full widget tree (header, scrolled content area, collapsed bar), the master index of all tab rows, group ordering, drag state, and all callback registrations. Provides the public API consumed by the rest of the application.

2. **`TabRow`** -- A single session's visual representation. A horizontal box containing an active indicator rail, a content column (title, subtitle, branch, PR, status pill, notification preview), a badge, an index overlay, and a close button. Owns its own status/badge/peek state via `Cell` for interior mutability.

3. **`TabGroupWidget`** -- A named, collapsible group. Contains a clickable header (toggle chevron, name label, add-tab button) and a `ListBox` for its tab rows. Manages its own collapse state and toggle callback.

4. **`CollapsedBar`** -- An alternative sidebar view for the collapsed state. Renders one Cairo-drawn colored circle per visible session, with status-based coloring and an accent ring for the active session.

The default section (ungrouped tabs) is handled directly by the `Sidebar` struct using a bare `ListBox` -- it has no header and no collapse capability.

## Modeling

```
Sidebar
|-- header_row          (GtkBox: "Sessions" label)
|-- scroll              (ScrolledWindow)
|   `-- content         (GtkBox, vertical)
|       |-- default_list        (ListBox: ungrouped tab rows)
|       |-- default_add_btn     (Button: "+ Add tab")
|       |-- groups_header       (Label: "Groups")
|       |-- [TabGroupWidget]*   (one per named group, in order)
|       `-- new_group_btn       (Button: "+ New Group")
`-- collapsed_bar       (CollapsedBar: dot circles)

TabGroupWidget
|-- container   (GtkBox, vertical)
|   |-- header  (GtkBox, horizontal)
|   |   |-- toggle_label    (Label: chevron)
|   |   |-- name_label      (Label: group name)
|   |   `-- add_btn         (Button: "+")
|   `-- list_box            (ListBox: tab rows for this group)

TabRow
|-- container   (GtkBox, horizontal)
|   |-- active_indicator    (GtkBox: left rail)
|   |-- content             (GtkBox, vertical)
|   |   |-- title_row       (GtkBox: folder_icon + title_label)
|   |   |-- subtitle_label  (Label: display path)
|   |   |-- branch_row      (GtkBox: branch_label + pr_label)
|   |   |-- status_label    (Label: status pill)
|   |   `-- preview_label   (Label: notification preview)
|   |-- badge_label         (Label: unread count)
|   |-- index_label         (Label: Alt-key index)
|   `-- close_btn           (Button: close)

CollapsedBar
|-- container   (GtkBox, vertical)
|   `-- scroll  (ScrolledWindow)
|       `-- content (GtkBox: DotEntry drawing areas)
```

### Key State Structures

- **`rows: HashMap<String, (TabRow, String)>`** -- Maps session ID to its `TabRow` widget and current group ID. This is the master index; all tab lookups go through it.
- **`groups: Vec<GroupEntry>`** -- Ordered list of named groups (id + display name). Determines visual ordering. The default group is implicit and not in this list.
- **`group_widgets: HashMap<String, TabGroupWidget>`** -- Maps group ID to the GTK widget. Separated from `groups` because the ordering vector and the widget lookup serve different access patterns.
- **`dragging_id: Rc<RefCell<String>>`** / **`dragging_group_id: Rc<RefCell<String>>`** -- Shared mutable strings that communicate the currently-dragged item's identity across drag source and drop target closures, since GTK4 drag-and-drop does not provide the dragged content during the `motion` signal.

## Data Flow

### 1. Tab Creation

**Entry**: `Sidebar::add_tab(session, after)` called by `SessionManager`.

**Process**:
1. Construct a new `TabRow` with the session's ID and title
2. Attach drag source (writes session ID to `dragging_id`)
3. Attach drop target (computes insertion position on drop)
4. Resolve the target `ListBox` from the session's `group_id` (default list or group widget's list)
5. If an `after` session ID is provided and exists in the same group, insert after it; otherwise append
6. Register in the `rows` HashMap
7. Add a dot to the collapsed bar

**Output**: Tab row appears in the sidebar at the correct position.

### 2. Tab Activation and Scroll-into-View

**Entry**: `Sidebar::set_active(session_id)` called by `SessionManager::switch_to`.

**Close button isolation**: The close button on each tab row uses a `GestureClick` that claims the event on press, preventing the click from reaching the parent container's tab-switch `GestureClick`. This ensures closing a tab does not also trigger a tab switch.

**Process**:
1. Mark all rows as inactive; mark the target row as active ("active" CSS class)
2. If the previously active row was peeking and no longer qualifies, clear its peek
3. If the new active tab is in a collapsed group, peek it; reconcile group visibility
4. Update the collapsed bar's active dot
5. Schedule a deferred idle callback (`glib::idle_add_local_once`) that:
   a. Guards: returns early if the sidebar is in collapsed (dot-bar) mode
   b. Looks up the tab row's parent `ListBoxRow` and computes its bounds relative to the scrollable content area via `compute_bounds`
   c. Compares the row's top/bottom edges against the `ScrolledWindow`'s vertical adjustment (`value` = scroll offset, `page_size` = viewport height)
   d. If the row is above the viewport, scrolls up so the row's top edge aligns with the viewport top
   e. If the row is below the viewport, scrolls down so the row's bottom edge aligns with the viewport bottom
   f. If already visible, does nothing

**Output**: The active tab is visually marked and scrolled into view if it was outside the visible area.

**Why deferred**: The peek toggling in steps 2-3 changes widget visibility, which invalidates layout-based position queries. The idle callback runs after GTK processes pending layout updates, ensuring accurate coordinates.

### 3. Tab Drag-and-Drop Reorder

**Entry**: User drags a tab row.

**Process**:
1. `DragSource::connect_prepare` writes the session ID into `dragging_id`
2. `DragSource::connect_drag_begin` creates a `WidgetPaintable` snapshot as drag icon, adds "dragging" CSS class
3. As the cursor moves over other tab rows, `DropTarget::connect_motion` checks whether the target is the same row or the row directly above (no-op positions), and shows a "drop-after" CSS indicator
4. On drop, the row is removed from its old `ListBox`, inserted at the computed position in the target `ListBox`, the target group is auto-expanded if collapsed, the `rows` HashMap is updated, and the `on_tab_moved` callback fires
5. `DragSource::connect_drag_end` cleans up CSS class and clears `dragging_id`

**Output**: The tab appears at the new position; the `SessionManager` is notified via callback to update persistence.

### 4. Group Collapse and Peek

**Entry**: User clicks a group header, or a session status changes.

**Process (collapse)**:
1. Toggle callback fires with `collapsed = true`
2. For every row in the group, check `should_peek()` (active, has badge, or is running)
3. Rows that should peek get `peeking = true`
4. `reconcile_peek_for_group` sets per-row GTK visibility: peeking rows visible, others hidden; if any row is peeking, the group's `list_box` stays visible (showing only peeked rows); if none are peeking, `list_box` is hidden entirely
5. Collapsed bar is refreshed

**Process (status change triggering peek)**:
1. `Sidebar::update_status` is called
2. If the session is now running, `peek_tab` marks it as peeking and reconciles the group
3. If the session is no longer running and does not otherwise qualify, `unpeek_tab` removes peek and reconciles

**Errors**: Peek operations silently no-op for default-group tabs (which have no collapse concept) or if the group is already expanded.

### 5. Sidebar Collapse/Expand

**Entry**: Toggle action (keyboard shortcut or UI button).

**Process**:
1. `set_sidebar_collapsed(true)`: Rebuilds the collapsed bar from `gather_dot_data()`, hides header and scroll, shows collapsed bar, sets width to 24px
2. `wire_sidebar_collapse` callback: Saves current paned position as `expanded_width`, snaps paned to `COLLAPSED_WIDTH`, disables wide handle, adds "sidebar-locked" CSS
3. A `notify::position` guard on the paned prevents drag-resize while collapsed (snaps back to `COLLAPSED_WIDTH`)
4. On expand: reverses all of the above, restores remembered width

**Output**: Sidebar toggles between full tab-row view and minimal dot-bar view.

### 6. Alt-Key Index Overlay

**Entry**: Alt key held down (detected by keyboard handler).

**Process**:
1. `show_tab_indices()` computes `ordered_visible_session_ids()` -- default group rows, then expanded group rows, then peeked tabs from collapsed groups
2. First clears all indices (handles stale state), then assigns indices 1-9 to the first 9 visible sessions
3. Index label becomes visible; close button becomes hidden (they occupy the same slot)
4. On Alt release, `hide_tab_indices()` clears all

## Key Decisions

### 1. Dual State for Group Ordering vs. Widget Lookup

**Choice**: Groups are stored as both an ordered `Vec<GroupEntry>` and a `HashMap<String, TabGroupWidget>`. The `Vec` determines display order; the `HashMap` provides O(1) widget access by ID.

**Why**: The sidebar needs both fast iteration in display order (for index assignment, ordered ID collection, group reordering) and fast random access by ID (for all update operations). A single structure cannot serve both efficiently.

**Alternatives Not Chosen**:
- `IndexMap` (ordered map) -- Would combine both but adds a dependency; the explicit separation also makes the ordering intent clearer in code.
- Single `Vec<(GroupEntry, TabGroupWidget)>` -- Would require linear scans for every widget lookup by ID.

**Consequences**: Group mutations must update both structures in sync, which adds maintenance burden. The code consistently does this, but it is a correctness invariant that the compiler cannot enforce.

**ADR/DES Candidate**: No -- this is a localized data structure choice, not a cross-cutting pattern.

### 2. GTK Widget Names as Identity Keys

**Choice**: Session IDs and group IDs are stored as GTK widget names (`set_widget_name` / `widget_name`), allowing drag-and-drop callbacks to identify which item is being interacted with by inspecting the widget tree.

**Why**: GTK4 drag-and-drop closures cannot easily capture arbitrary per-widget state. By encoding the ID in the widget name, any callback that has access to the widget can recover the domain identity without maintaining a separate reverse-lookup map.

**Alternatives Not Chosen**:
- Storing IDs in closure captures -- Would require unique closures per row, which is already done for some callbacks but becomes unwieldy for drop targets that need both source and target IDs.
- Custom GObject properties -- More idiomatic in GObject terms but heavier and more verbose in Rust.

**Consequences**: Widget names become load-bearing identifiers, not just debugging aids. If any code changes a widget name after creation, lookups break silently. This convention must be understood by all contributors.

**ADR/DES Candidate**: Yes -- **DES candidate**. This is a repeatable pattern ("widget name as domain key") used across tab rows, group widgets, and the sidebar itself.

### 3. Shared `Rc<RefCell<String>>` for Drag State Communication

**Choice**: A shared mutable string (`dragging_id`, `dragging_group_id`) communicates which item is being dragged. The drag source writes to it on `prepare`; drop targets read it on `motion` and `drop`.

**Why**: GTK4's `DropTarget::connect_motion` signal does not provide the drag content -- only `connect_drop` does. But motion handling needs to know the dragged item's identity to show or suppress visual indicators (e.g., don't show "drop-after" on the row being dragged, or on the row directly above it). The shared string bridges this gap.

**Alternatives Not Chosen**:
- Ignoring the identity during motion -- Would result in incorrect visual indicators (showing drop zones on the dragged item itself or in no-op positions).
- Using a global/thread-local -- Less explicit and harder to reason about ownership.

**Consequences**: Two separate shared strings exist (one for tabs, one for groups) to prevent cross-contamination. The tab drop target explicitly guards against an empty `dragging_id` to reject group drags that propagate down.

**ADR/DES Candidate**: Yes -- **DES candidate**. This "shared drag identity" pattern is reusable for any GTK4 drag-and-drop scenario where motion handling needs source identity.

### 4. Type-Differentiated Drop Targets (GString vs. Variant)

**Choice**: Tab drag-and-drop uses `GString` as the content type; group drag-and-drop uses `Variant`. This ensures a tab drop target never accidentally accepts a group drag, and vice versa.

**Why**: Both tab rows and group widgets live in the same widget tree. Without type differentiation, a drop target on a tab row could intercept a group being dragged over it (or vice versa), leading to incorrect behavior. Using distinct GObject types in the `DropTarget` constructor causes GTK4 to route drags only to compatible targets.

**Alternatives Not Chosen**:
- Single content type with a discriminator field -- Would require every drop handler to inspect the payload and decide whether to accept, adding fragile branching logic.
- Separate drag zones that don't overlap -- Not feasible given the visual nesting.

**Consequences**: The drag source for groups must wrap the ID in a `Variant` (`to_variant().to_value()`), and the drop handler must unwrap it (`value.get::<Variant>()` then `variant.get::<String>()`). This double-wrapping is unintuitive but necessary for type safety.

**ADR/DES Candidate**: Yes -- **DES candidate**. "Type-differentiated drop targets for overlapping drag zones" is a pattern applicable to any GTK4 UI with multiple draggable item types.

### 5. Peek as a Per-Row Boolean with Group-Level Reconciliation

**Choice**: Each `TabRow` has a `peeking: Cell<bool>` flag. When a collapsed group needs to show certain tabs, their peek flags are set, and then a `reconcile_peek_for_group` function walks all rows in the group to set per-row GTK visibility and determine whether the group's `list_box` should be visible at all.

**Why**: Peek behavior is triggered from multiple sources (group collapse, status change, active tab change, tab removal). Rather than each trigger independently computing visibility, the pattern is: set/clear per-row peek flags, then call reconcile. This centralizes the visibility logic.

**Alternatives Not Chosen**:
- Reactive/signal-based approach -- GTK4 does not natively support reactive computed properties. Implementing observability would add significant complexity.
- Maintaining a separate "visible set" per group -- Would duplicate truth and risk divergence from the actual `peeking` flags on rows.

**Consequences**: The reconcile function must be called after every peek flag mutation. Missing a reconcile call leaves the UI in an inconsistent state. The code handles this by providing two variants: `reconcile_group_peek` (reconcile + refresh collapsed bar) and `reconcile_group_peek_only` (reconcile without refresh, for batching).

**ADR/DES Candidate**: Yes -- **DES candidate**. "Flag-then-reconcile" is a general UI state management pattern that could apply to other composite widget behaviors.

## System Behavior

### Tab Row Displays Rich Metadata

- **Given** a session with a CWD of `/home/user/project`
- **When** the sidebar receives a `update_cwd` call
- **Then** the tab row shows "project" as the title, a folder icon, and the display path as a subtitle

### Collapsed Group Peeks Active Tab

- **Given** a named group containing sessions A, B, and C, with A being the active session
- **When** the user clicks the group header to collapse it
- **Then** only session A's tab row is visible under the collapsed header; B and C are hidden

### Status Change Triggers Peek

- **Given** a collapsed group where all tabs are idle and hidden
- **When** session B transitions to `Running` status
- **Then** session B's tab row appears (peeks) under the collapsed header

### Peek Revoked When No Longer Qualifying

- **Given** a collapsed group with session B peeking because it was `Running`
- **When** session B returns to `Idle` and has no unread badge and is not active
- **Then** session B's peek is removed; if no other tabs are peeking, the group's list hides entirely

### Tab Moved Between Groups via Drag-and-Drop

- **Given** session A in the default group and a named group "Backend"
- **When** the user drags session A's tab row and drops it onto a row in "Backend"
- **Then** session A moves to "Backend" after the target row, the `on_tab_moved` callback fires with the new group and position, and if "Backend" was collapsed it auto-expands

### Sidebar Collapse Preserves Width

- **Given** the sidebar is expanded at 280px width
- **When** the user collapses the sidebar
- **Then** the sidebar snaps to 24px, the paned handle is locked, and the dot bar appears with one circle per visible session
- **When** the user expands the sidebar
- **Then** it restores to 280px and the full tab-row view returns

### Alt Index Overlay Respects Visibility

- **Given** 3 default tabs and a collapsed group with 2 tabs (one peeking)
- **When** the user holds Alt
- **Then** indices 1-4 appear on the 3 default tabs and the 1 peeked tab; the hidden tab in the collapsed group receives no index

### Tab Row Shows Pointer Cursor

- **Given** any tab row in the sidebar
- **When** the user hovers over it
- **Then** the cursor changes to a pointer hand, indicating clickability

### PR Label Shows Underline on Ctrl+Hover

- **Given** a tab row with a PR link displayed
- **When** the user holds Ctrl and hovers over the PR label
- **Then** the PR text gains an underline, indicating Ctrl+click will open the PR URL
- **When** the user moves the cursor away or releases Ctrl (detected on next mouse movement)
- **Then** the underline is removed

### Browser Tab Row Shows Browser Metadata

- **Given** a browser pane is focused in a session, **when** the sidebar tab row is displayed, **then** it shows a globe icon (replacing the folder icon), the page title as the title, and the current URL as the subtitle; git branch and PR labels are hidden.
- **Given** a session with mixed shell and browser panes, **when** focus switches from a browser pane to a shell pane, **then** the tab row reverts to folder icon, CWD-based display, and git branch/PR visibility. **When** focus switches back to the browser pane, **then** it returns to globe icon display.
- **Given** a browser pane's URL or title changes (reported via `update_browser_display`), **when** that pane is focused, **then** the sidebar immediately reflects the new values.

## Notes

- **Limitation**: The PR label underline is removed on Ctrl release only when the mouse moves; if the user releases Ctrl while stationary over the label, the underline persists until the next mouse event. This avoids adding a key-event controller for a minor cosmetic edge case.
- **Uncertainties**: The `expanded_width` field is initialized to 0 and then set by `wire_sidebar_collapse` in a separate wiring step. If `effective_sidebar_width` is called before wiring, it returns 0. This ordering dependency is not enforced by the type system.
- **Assumptions**: The drag-and-drop "above dragged" guard (skipping indicators on the row directly before the dragged item) relies on GTK widget tree sibling order matching visual order, which holds for `ListBox` children.
- **Areas needing clarification**: The `on_group_expanded` callback is registered but its consumer is not in the sidebar module -- its purpose (likely scrolling or focusing) would need to be traced through `app/mod.rs` to fully document.
- **Potential improvement**: The `reconcile_peek_for_group` function iterates through `ListBox` rows by index, matching widget names back to the `rows` HashMap. This two-phase lookup (widget tree -> HashMap) could be simplified if group membership were tracked as a list of session IDs per group, but the current approach avoids maintaining a third data structure.
