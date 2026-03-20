# seemux

A GTK4 terminal multiplexer for Linux with Claude Code integration, inspired by [cmux](https://github.com/manaflow-ai/cmux).

## Features

- **Tabbed terminal** with vertical sidebar showing title, git branch, status pill, and notification badges
- **Collapsible sidebar** — collapse to a minimal 24px bar with colored status dots; expand back to full view (Ctrl+Shift+B)
- **Split panes** — horizontal and vertical splits within any tab, with click-to-focus and directional navigation
- **Tab groups** — organize tabs into named, collapsible groups with drag-and-drop reordering
- **Claude Code integration** — real-time session status (Running, Needs Input, Completed, Error) and desktop notifications via a Claude Code plugin, with automatic session resume on restart
- **Agent Teams support** — tmux shim intercepts Claude Code team commands, spawning subagent sessions as seemux tabs in dedicated team groups
- **PR detection** — automatically detects PRs created by Claude via `gh pr` and shows a clickable badge on the tab
- **Dropdown mode** — quake-style terminal toggled via `seemux toggle` or a global shortcut
- **Editor integration** — Ctrl+Click or right-click file:// links to open in neovim with per-terminal socket reuse
- **Session persistence** — tabs, groups, splits, working directories, sidebar state, and Claude sessions restored on restart
- **Notification peek** — tabs with notifications automatically peek out of collapsed groups
- **Themes** — Catppuccin Mocha (default) and Dracula

## Requirements

- GTK4 (4.12+)
- VTE4 (0.76+)
- Rust 2024 edition
- `socat` (for Claude Code hook communication)

## Install

```bash
cargo build --release
```

The binary will be at `target/release/seemux`.

## Usage

```bash
seemux           # Launch the terminal
seemux toggle    # Toggle dropdown window (bind to a global hotkey)
```

## Claude Code Integration

Seemux includes a Claude Code plugin that sends session lifecycle events to seemux via Unix socket, enabling real-time status indicators and notification badges on your terminal tabs.

### Plugin Setup

```bash
# Add the seemux plugin marketplace
claude /plugins marketplace add github asermax/seemux

# Install the hooks plugin
claude /plugins install seemux-hooks
```

That's it. When you run `claude` inside a seemux terminal, the plugin detects the `$SEEMUX_SOCKET` env var and sends events automatically. Your existing Claude Code settings and hooks are preserved — the plugin hooks are additive.

### How It Works

```
Claude Code ──hooks──> seemux-hook.sh ──Unix socket──> seemux ──> UI updates
```

1. Seemux sets `SEEMUX_SOCKET` and `SEEMUX_SESSION_ID` env vars in each terminal
2. The plugin's hook scripts read these env vars and send event JSON to the socket
3. Seemux receives events on its background thread and updates the sidebar:
   - **Status pill** — Running (blue), Needs Input (yellow), Completed (green), Error (red), Idle (gray)
   - **Notification badge** — unread count with preview text
   - **Desktop notifications** — for Permission, Error, and Waiting events when the tab is not active
4. On restart, Claude sessions are automatically resumed via `claude --resume`

## Keyboard Shortcuts

### Tabs

| Shortcut | Action |
|----------|--------|
| Ctrl+T | New tab (inherits active terminal's CWD) |
| Ctrl+Shift+W | Close pane (or tab if single pane) |
| Ctrl+Tab / Ctrl+Page Down | Next tab |
| Ctrl+Shift+Tab / Ctrl+Page Up | Previous tab |
| Alt+Page Down | Next tab |
| Alt+Page Up | Previous tab |
| Alt+1-9 / Ctrl+1-9 | Jump to tab by index |
| Hold Alt | Show tab index numbers in sidebar |

### Sidebar

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+B | Toggle sidebar collapse/expand |

### Groups

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+G | New group |
| Ctrl+Alt+Page Down | Next group |
| Ctrl+Alt+Page Up | Previous group |

### Notifications

| Shortcut | Action |
|----------|--------|
| Alt+Shift+Page Down | Next tab with notifications (falls back to next tab) |
| Alt+Shift+Page Up | Previous tab with notifications (falls back to previous tab) |

### Split Panes

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+H | Split horizontal |
| Ctrl+Shift+E | Split vertical |
| Alt+h | Focus pane left |
| Alt+j | Focus pane down |
| Alt+k | Focus pane up |
| Alt+l | Focus pane right |

### Terminal

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+C | Copy |
| Ctrl+Shift+V | Paste |
| Ctrl+Click | Open URL under cursor (file:// opens in editor, others in browser) |

### Window

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+N | New window |

### Context Menu

Right-click on the terminal area for: Copy, Paste, Split Horizontal, Split Vertical, Close. When clicking on a URL, additional options appear:
- **file:// links (text files)**: Open in Editor (neovim), Open with external App
- **Other URLs**: Open URL

Right-click on a tab for: Rename, Close, Close Others.

Right-click on a group header for: Delete Group.

### Drag and Drop

Tabs can be dragged between groups and reordered within a group.

## Configuration

Config file at `~/.config/seemux/config.toml`:

```toml
font_family = "Monospace"         # Font family
font_size = 13                    # Font size in points
scrollback_lines = 10000          # Terminal scrollback buffer
sidebar_width = 200               # Sidebar width in pixels
color_scheme = "catppuccin-mocha" # "catppuccin-mocha" or "dracula"
dropdown_width_percent = 90       # Dropdown window width (% of screen)
dropdown_height_percent = 50      # Dropdown window height (% of screen)
dropdown_animation_ms = 200       # Dropdown slide animation duration
sidebar_collapsed = false         # Start with sidebar collapsed
```

## File Locations

| File | Path |
|------|------|
| Config | `~/.config/seemux/config.toml` |
| Session state | `~/.local/state/seemux/sessions.json` |
| Runtime socket | `$XDG_RUNTIME_DIR/seemux/seemux.sock` |
