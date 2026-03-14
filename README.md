# seemux

A GTK4 terminal multiplexer for Linux with Claude Code integration, inspired by [cmux](https://github.com/manaflow-ai/cmux).

## Features

- **Tabbed terminal** with vertical sidebar showing title, git branch, status, and notification badges
- **Split panes** — horizontal and vertical splits within any tab
- **Tab groups** — organize tabs into named, collapsible groups
- **Claude Code integration** — automatically hooks into Claude sessions to show real-time status (running, needs input, error, completed) and notification badges
- **Dropdown mode** — quake-style terminal toggled via `seemux toggle`
- **Session persistence** — tabs, groups, and working directories restored on restart
- **Themes** — Catppuccin Mocha (default) and Dracula

## Requirements

- GTK4 (4.12+)
- VTE4 (0.76+)
- Rust 2024 edition

## Install

```bash
cargo build --release
```

The binary will be at `target/release/seemux`.

## Usage

```bash
seemux           # Launch the terminal
seemux toggle    # Toggle dropdown window
```

## How Claude Integration Works

On startup, seemux generates wrapper scripts in `$XDG_RUNTIME_DIR/seemux/bin/` and prepends that directory to `PATH`. When you run `claude` inside a seemux terminal, the wrapper:

1. Finds the real `claude` binary
2. Injects hook configuration that sends events via Unix socket
3. Seemux receives events and updates tab status/notifications in real-time

No manual hook configuration needed — it works transparently.

## Keyboard Shortcuts

### Tabs

| Shortcut | Action |
|---|---|
| Ctrl+T | New tab |
| Ctrl+Shift+W | Close pane (or tab if single pane) |
| Ctrl+Tab / Ctrl+Shift+Tab | Cycle tabs forward / backward |
| Ctrl+PgDown / Ctrl+PgUp | Next / previous tab |
| Ctrl+Shift+PgDown / Ctrl+Shift+PgUp | Next / previous group |
| Alt+1–9 | Jump to tab by index |

### Split Panes

| Shortcut | Action |
|---|---|
| Ctrl+Shift+H | Split horizontal |
| Ctrl+Shift+E | Split vertical |

### Terminal

| Shortcut | Action |
|---|---|
| Ctrl+Shift+C | Copy |
| Ctrl+Shift+V | Paste |

### Window

| Shortcut | Action |
|---|---|
| Ctrl+Shift+N | New window |

## Configuration

Config file at `~/.config/seemux/config.toml`:

```toml
font = "monospace 11"
scrollback_lines = 10000
sidebar_width = 220
color_scheme = "catppuccin-mocha"  # or "dracula"
```
