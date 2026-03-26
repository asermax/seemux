# Delta Inventory

Deltas (work items) extracted from VISION.md for Seemux.

## Status Tracking

Deltas track their progress through the development workflow using a status field:

- **✗ Defined** - Delta extracted and documented (initial state)
- **⧗ Spec** - Specification in progress (`/spec-delta` started)
- **✓ Spec** - Specification complete (`/spec-delta` done)
- **⧗ Design** - Design rationale in progress (`/design-delta` started)
- **✓ Design** - Design complete (`/design-delta` done)
- **⧗ Plan** - Implementation plan in progress (`/plan-delta` started)
- **✓ Plan** - Implementation plan complete (`/plan-delta` done)
- **⧗ Implementation** - Delta implementation in progress (`/implement-delta` started)
- **✓ Implementation** - Delta complete and tested (`/implement-delta` done)
- **✓ Reconciled** - Feature documentation updated (`/reconcile-delta` done)

Commands automatically update status as they progress. To manually update:
```bash
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py status set DELTA-ID "STATUS"
```

Query status:
```bash
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py status list                    # All deltas
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py status list --complexity Easy  # Filter by complexity
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py status show DELTA-ID           # Detailed view
```

## Priority Tracking

Deltas have a priority level (1-5) that determines their urgency:

| Level | Label | Description |
|-------|-------|-------------|
| 1 | Critical | Blocks release, must do now |
| 2 | High | Important, needed soon |
| 3 | Medium | Standard priority (default) |
| 4 | Low | Nice to have |
| 5 | Backlog | Someday/maybe |

Set priority:
```bash
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py priority set DELTA-ID LEVEL
```

List by priority:
```bash
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py priority list                  # Grouped by priority
python ${CLAUDE_PLUGIN_ROOT}/scripts/deltas.py priority list --level 1        # Filter by level
```

---

## Deltas

### DLT-001: Simplify sessions to single-terminal tabs
**Status**: ✗ Defined
**Depends on**: None
**Priority**: 2 (High)
**Complexity**: Medium
**Description**: The current split pane system (SplitView/SplitTree) adds complexity that goes unused. This delta removes the split pane infrastructure and ensures each session is always a single terminal, simplifying the session model and persistence format. This prepares the architecture for a layout system where multiple tabs can be arranged side-by-side.

### DLT-002: Arrange tabs side-by-side in persistent layouts
**Status**: ✗ Defined
**Depends on**: DLT-001
**Priority**: 2 (High)
**Complexity**: Hard
**Description**: Users working with multiple terminals need to view them simultaneously without losing independent tab identity. This delta introduces layouts — arrangements of tabs displayed side-by-side — created via right-click menu to add a tab in a specific direction. Each tab remains independently selectable in the sidebar, pane sizes are user-adjustable, a tab can participate in multiple layouts, and layouts persist across restarts. A keybinding toggles layout creation/destruction from the current view.

### DLT-003: Auto-create layouts for spawned child tabs
**Status**: ✗ Defined
**Depends on**: DLT-002
**Priority**: 3 (Medium)
**Complexity**: Medium
**Description**: When a process spawns a child tab (such as an editor or a Claude teammate), users must manually arrange it alongside the originating tab. This delta automatically places spawned child tabs into a layout adjacent to their parent, reducing manual arrangement for common workflows like editing files or monitoring teammates.

### DLT-004: Open web pages in browser tabs
**Status**: ✗ Defined
**Depends on**: DLT-002
**Priority**: 3 (Medium)
**Complexity**: Medium
**Description**: Users need to view web content alongside terminal sessions without leaving seemux. This delta adds browser tabs that render web pages inside the terminal, using a terminal-based browser such as Carbonyl. Sessions gain a type distinction (shell vs browser) so the sidebar can display browser-appropriate information (URL and page title instead of working directory, distinct icon). Browser tabs are independent sessions that participate in the layout system, enabling workflows like viewing documentation side-by-side with a coding terminal.

---

## Notes

- DLT-001 is a prerequisite cleanup that simplifies the session model before introducing layouts
- DLT-002 has open design questions to resolve during spec: whether layouts are independent entities vs attached to tabs, and how layout activation interacts with single-tab selection
- DLT-003 depends on integration points with Agent Teams (tmux shim) and editor spawning mechanisms
- DLT-004 initial implementation idea: use Carbonyl (terminal-based Chromium browser) spawned inside VTE terminals
