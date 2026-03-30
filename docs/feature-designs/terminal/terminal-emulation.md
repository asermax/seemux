# Design: Terminal Emulation and Split Panes

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/terminal/terminal-emulation.md](../../feature-specs/terminal/terminal-emulation.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/terminal/`.
Retrofit date: 2026-03-24
Decisions discovered: Data-first layout with widget rebuild (DES candidate), Scroll guard approach (ADR candidate), Safe GTK4 Paned teardown (DES candidate)

---

## Purpose

This document explains the design rationale for terminal emulation and split pane management: the modeling choices, data flow, component responsibilities, and architectural approach.

## Problem Context

Seemux needs a fully functional terminal emulator embedded in GTK4, with arbitrary split pane layouts per session. Key constraints:

- Single-threaded GTK event loop (no Arc/Mutex)
- VTE4 silently mutates scroll state during screen switches and buffer growth
- GTK Paned widgets retain internal child references requiring careful teardown
- Split layouts must serialize to JSON for persistence across restarts

## Design Overview

Two components with cleanly separated responsibilities:

1. **VteTerminal** — thin wrapper around VTE4 encapsulating configuration, input interception (Shift+Enter), URL detection, scroll guard, and process spawning. Exposes a callback API so callers never interact with VTE4 signals directly.

2. **SplitView** — manages per-session pane layout using a private binary tree (`SplitTree`) and a flat `HashMap` of pane IDs to terminals. All tree mutations happen on data first; the GTK widget tree is rebuilt on demand.

## Modeling

```
SplitView
├── panes: HashMap<String, Rc<VteTerminal>>   (flat registry)
├── tree: SplitTree                            (layout structure)
└── focused_pane_id: String                    (current focus)

SplitTree (private enum)
├── Leaf(pane_id)
└── Split { orientation, first, second }

VteTerminal
├── container: gtk4::Box      (terminal + scrollbar)
├── terminal: vte4::Terminal   (VTE4 widget)
└── spawned: Cell<bool>        (spawn-once guard)
```

The tree stores only pane ID strings; actual `VteTerminal` instances live in the HashMap. Tree transformations are pure data operations. The GTK widget tree is ephemeral — rebuilt from scratch on layout changes.

Serialization uses `SavedSplitNode` (from `config.rs`), mirroring `SplitTree` but replacing pane IDs with optional CWD strings. On restore, fresh UUIDs are generated for every pane.

## Data Flow

### Terminal Creation

1. Resolve color scheme from config via `theme::get_scheme` (fallback: Catppuccin Mocha)
2. Build VTE Terminal with scrollback, scroll-on-output enabled, scroll-on-keystroke, font, colors, bold-is-bright, no bell
3. Install Shift+Enter key controller (capture phase, feeds kitty escape `\x1b[13;2u`)
4. Install URL regex matcher + enable OSC 8 hyperlinks
5. Create scrollbar bound to VTE's vadjustment
6. Install scroll guard controllers
7. Pack into horizontal Box

### Scroll Guard

`scroll_on_output` is enabled so VTE natively keeps at-bottom terminals at the bottom when new output arrives (including in background tabs). The scroll guard handles the complementary case: preserving user scroll position during VTE internal adjustments (cursor movement, screen switches, ring growth) that would otherwise jump the viewport.

Three event controllers detect user-initiated scrolling: mouse wheel, keyboard Shift+Page/Home/End, and scrollbar drag — all in capture phase. On `value_changed`:

- If user-initiated: record offset-from-bottom (or clear if at bottom)
- If VTE-initiated and user is scrolled up: restore saved offset
- A `restoring` flag prevents re-entrant loops

On `changed` (bounds update): restore, then nudge adjustment +1/-1 to force VTE display re-sync.

### Pane Splitting

1. Generate UUID for new pane
2. Create VteTerminal, insert into panes map
3. Mutate SplitTree: focused leaf becomes Split node with original as first, new as second
4. Update focused pane to new pane
5. Return (pane_id, terminal) to caller for signal wiring
6. Caller calls `rebuild_in_stack` separately

### Widget Rebuild

1. Grab focus on stack (away from tree being torn down)
2. Recursively clear all Paned children via `set_start_child(None)` / `set_end_child(None)` / `set_focus_child(None)`
3. Remove old widget from stack
4. Walk SplitTree to build new Paned hierarchy
5. Add to stack and make visible

### Serialization Round-Trip

- **Save**: Walk SplitTree, map each leaf's pane ID to CWD, emit SavedSplitNode
- **Restore**: Walk SavedSplitNode, create fresh UUID + VteTerminal per leaf, build SplitTree, collect (pane_id, cwd) pairs for deferred spawning

## Key Decisions

### Separated Data Tree and Widget Tree

**Choice**: SplitTree is pure data (pane ID strings only). GTK widget tree rebuilt from scratch on every layout change.

**Why**: GTK Paned has complex internal state making in-place re-parenting fragile. Rebuilding from data produces clean widget hierarchies.

**Alternatives Considered**:
- In-place widget mutation: rejected — GTK Paned retains internal references causing warnings/crashes
- Flat list with computed positions (tmux-style): rejected — binary tree naturally models nested splits and simplifies navigation

**Consequences**:
- Pro: Tree operations are simple; no GTK coupling in data mutations; reliable teardown
- Con: Every split/close triggers full widget rebuild; pane divider positions not preserved across rebuilds

### Scroll Guard with User-Interaction Detection

**Choice**: Multi-controller guard distinguishing user scrolling from VTE-internal adjustments via capture-phase event interception, combined with VTE's native `scroll_on_output` for at-bottom terminals.

**Why**: VTE4 mutates scroll adjustment during screen switches and buffer growth. No "is this a user scroll" API exists. `scroll_on_output` handles the at-bottom-on-new-output case natively; the scroll guard handles cursor-movement and internal adjustment cases.

**Alternatives Considered**:
- Disabling scroll-on-output entirely: insufficient — VTE still jumps on internal state changes, and at-bottom terminals lose track of position in background tabs
- Forking VTE to expose user-scroll semantics: rejected as maintenance burden
- Timer-based debounce: rejected as unreliable

**Consequences**:
- Pro: Users can scroll up in active terminals without viewport jumping; background terminals stay at bottom on new output
- Con: Intricate implementation (four controllers, four flags, two signal handlers); the +1/-1 nudge is a VTE rendering workaround

### Safe GTK4 Paned Teardown

**Choice**: Recursively set start_child, end_child, focus_child to None before removing Paned widgets, rather than calling unparent() on children.

**Why**: GTK4 Paned maintains internal child references. Direct unparent() leaves inconsistent state producing warnings and potential crashes.

**Consequences**:
- Pro: Clean teardown with no GTK warnings
- Con: Added complexity; developers must follow this pattern

## System Behavior

### Directional Navigation

Given `Split(H, A, Split(V, B, C))` with A focused, navigating Right moves focus to B (first pane of right subtree). Navigation searches ancestor nodes for a sibling in the requested direction, then descends to the nearest leaf.

### Scroll Guard Deferred Restoration

When VTE switches to alternate screen and bounds shrink below stored offset, restoration is deferred until bounds return to normal, preventing incorrect positioning.

---

## Notes

- `check_url_at` is a static method taking a raw `vte4::Terminal` reference, slightly breaking the wrapper's encapsulation — pragmatic compromise for action code that has widget access but not wrapper access.
- The scroll guard's nudge workaround may become unnecessary if VTE fixes the display desync upstream.
