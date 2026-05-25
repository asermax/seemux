# Design: Terminal Emulation and Split Panes

<!-- This design describes the current implementation approach. Updated through delta reconciliation. -->

**Feature Spec**: [../../feature-specs/terminal/terminal-emulation.md](../../feature-specs/terminal/terminal-emulation.md)
**Status**: Current

## Retrofit Note

This design was created from existing code at `src/terminal/`.
Retrofit date: 2026-03-24
Decisions discovered: Data-first layout with widget rebuild (DES candidate), Safe GTK4 Paned teardown (DES candidate)

---

## Purpose

This document explains the design rationale for terminal emulation and split pane management: the modeling choices, data flow, component responsibilities, and architectural approach.

## Problem Context

Seemux needs a fully functional terminal emulator embedded in GTK4, with arbitrary split pane layouts per session. Key constraints:

- Single-threaded GTK event loop (no Arc/Mutex)
- GTK Paned widgets retain internal child references requiring careful teardown
- Split layouts must serialize to JSON for persistence across restarts

## Design Overview

Two components with cleanly separated responsibilities:

1. **VteTerminal** — thin wrapper around VTE4 encapsulating configuration, input interception (Shift+Enter), URL detection, clipboard auto-copy and middle-click paste, and process spawning. Exposes a callback API so callers never interact with VTE4 signals directly.

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
2. Build VTE Terminal with scrollback, scroll-on-output disabled, scroll-on-keystroke, font, colors, bold-is-bright, no bell
3. Install Shift+Enter key controller (capture phase, feeds kitty escape `\x1b[13;2u`)
4. Install URL regex matcher + enable OSC 8 hyperlinks
5. Connect `selection-changed` to copy to CLIPBOARD when a selection exists; install a button-2 `GestureClick` (default Bubble propagation) that calls `paste_clipboard()` on press
6. Create scrollbar bound to VTE's vadjustment
7. Pack into horizontal Box

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

### Safe GTK4 Paned Teardown

**Choice**: Recursively set start_child, end_child, focus_child to None before removing Paned widgets, rather than calling unparent() on children.

**Why**: GTK4 Paned maintains internal child references. Direct unparent() leaves inconsistent state producing warnings and potential crashes.

**Consequences**:
- Pro: Clean teardown with no GTK warnings
- Con: Added complexity; developers must follow this pattern

## System Behavior

### Directional Navigation

Given `Split(H, A, Split(V, B, C))` with A focused, navigating Right moves focus to B (first pane of right subtree). Navigation searches ancestor nodes for a sibling in the requested direction, then descends to the nearest leaf.

---

## Notes

- `check_url_at` is a static method taking a raw `vte4::Terminal` reference, slightly breaking the wrapper's encapsulation — pragmatic compromise for action code that has widget access but not wrapper access.
- `paste_clipboard()` is a documented silent no-op when the clipboard is empty, so middle-click in those situations safely produces no input and no log output without an explicit guard.
- VTE's GTK4 widget does not bind middle-click to paste internally; `setup_middle_click_paste` wires the gesture itself. The gesture uses default (Bubble) propagation and does not claim the event, so VTE still receives the press and can manage its own selection state — which also leaves room for a future delta to refine interaction with terminal mouse-reporting modes (tmux/vim/htop).

### URL Detection Across Soft-Wrapped Lines

Long URLs printed in the terminal often wrap across visual rows. VTE's regex matcher only matches against single visual rows, and the URL pattern requires a prefix (`https://`, `www.`, `./`, `../`), so continuation rows of a wrapped URL never match independently. To recover the full URL on click, `check_url_at` reconstructs the logical line at the click position:

1. **Pixel → cell.** `vadjustment().value().round() as i64` gives the viewport's top row (VTE's vadjustment is row-indexed). `char_width()` and `char_height()` map pixel offsets to column and row deltas. Buffer extents come from `vadjustment().lower()`/`.upper()` since vte4 0.10 does not expose `first_row`/`last_row`.

2. **Soft-wrap detection.** A single-row probe `text_range_format(Format::Text, r, 0, r, column_count - 1)` returns the row's content followed by `\n` iff the row ends with a hard newline; for soft-wrapped rows no `\n` is appended. Walking up while the previous row is soft-wrapped (and down while the current row is) yields the logical-line bounds.

3. **Logical line reconstruction.** Per-row probe results are concatenated (each trimmed of trailing `\n`) into one `String`, with each row's starting byte offset recorded.

4. **Click offset.** A second probe `text_range_format(..., row, 0, row, col - 1)` gives the byte length of the click row's prefix. Computing the offset from VTE-provided byte lengths (rather than from cell arithmetic) is correct in the presence of multibyte chars and wide cells (CJK, emoji), where cells, characters, and bytes do not coincide.

5. **Match.** A Rust `regex::Regex` compiled from the same `URL_REGEX` string used for VTE's PCRE2 hover-cursor highlighting is run on the logical line; the URL whose half-open `[start, end)` byte interval contains the click offset is returned. The half-open interval mirrors VTE's per-cell `check_match_at` semantics, preserving boundary behavior for single-row URLs.

OSC 8 hyperlinks remain handled by `check_hyperlink_at` as a fast-path before any reconstruction — VTE associates the OSC 8 URI with each cell of the anchor span, so this works for both single-row and wrapped hyperlinks.

The walk and the URL-finding logic are factored as pure functions (`logical_line_bounds`, `find_url_in_logical_line`) so they are unit-testable without GTK.
