# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Seemux is a GTK4-based terminal multiplexer designed for Claude Code integration. It provides a tabbed terminal interface with split panes, session groups, notifications via Claude hooks, and a dropdown/quake-style terminal mode.

Inspired by [cmux](https://github.com/manaflow-ai/cmux) (macOS, Swift/AppKit/libghostty) — seemux is the Linux-native counterpart built from scratch with the same conceptual architecture but no shared code.

**Stack:** Rust 2024 edition, GTK4 (v4_12), VTE4 (v0_76)

## Build & Development Commands

```bash
cargo build                # Debug build
cargo build --release      # Release build
cargo test                 # Run all tests
cargo test <test_name>     # Run a single test
cargo run                  # Run the app
cargo run -- toggle        # Trigger dropdown toggle via CLI
```

The build script (`build.rs`) compiles GTK resources via `glib-build-tools` — the resource manifest is at `resources/seemux.gresource.xml` (embeds `style.css`).

## Architecture

### Data Flow

1. **Startup**: `AppState` loads config, starts `HookServer` (Unix socket), generates Claude wrapper scripts. Main window builds sidebar + terminal stack, restores saved sessions/groups.
2. **Tab creation**: `SessionManager` creates a `Session` (UUID-identified), wraps a `VteTerminal` in a `SplitView` leaf, adds a `TabRow` to the sidebar.
3. **Pane splitting**: `SplitView` rebuilds its `SplitNode` tree (Leaf | Split), creating new terminals.
4. **Hook events**: Claude sends JSON via Unix socket → `HookServer` thread → mpsc channel → main thread polls every 100ms → `HookHandler` parses → status/notification updates.
5. **Persistence**: On window close, sessions/groups saved to JSON. On startup, restored from disk.

### Key Modules

| Module | Purpose |
|---|---|
| `app.rs` | Main window layout, keyboard shortcuts, hook polling |
| `app_state.rs` | Shared state: Config, hook receiver, socket/bin paths |
| `session/manager.rs` | Session lifecycle, split pane management, tab switching |
| `session/mod.rs` | Session struct (id, title, status, cwd, group) |
| `terminal/vte_terminal.rs` | VTE4 wrapper: config, shell spawning, signal handlers |
| `terminal/split_view.rs` | Tree structure for split panes (SplitNode enum) |
| `sidebar/mod.rs` | Sidebar container with default + named groups |
| `sidebar/tab_row.rs` | Tab widget: title, status pill, git branch, badge, close |
| `sidebar/tab_group.rs` | Collapsible group header |
| `config.rs` | Config (TOML) + SessionState (JSON) persistence |
| `notifications/hook_server.rs` | Unix socket listener for Claude hook events |
| `notifications/hook_handler.rs` | Event parsing → session status mapping |
| `notifications/mod.rs` | NotificationStore: unread counts per session |
| `claude/hook_script.rs` | Generates runtime bash scripts for hook injection |
| `theme.rs` | Color schemes (Catppuccin Mocha, Dracula), runtime CSS generation |
| `dropdown.rs` | Quake-style dropdown window with animated revealer |
| `git.rs` | Async git branch detection for tabs |
| `cli.rs` | CLI argument parsing (`seemux toggle`) |

### State Management Pattern

Shared mutable state uses `Rc<RefCell<T>>` throughout (single-threaded GTK event loop). Cross-thread communication (hook server → main) uses `mpsc` channels.

### File Locations

- **Config**: `~/.config/seemux/config.toml`
- **Session state**: `~/.local/state/seemux/sessions.json`
- **Runtime socket**: `$XDG_RUNTIME_DIR/seemux/seemux.sock`
- **Runtime bin (Claude wrapper scripts)**: `$XDG_RUNTIME_DIR/seemux/bin/`

## Testing

Tests live alongside source code in `#[cfg(test)]` modules within:
- `src/session/mod.rs` — session creation, status labels, CSS classes
- `src/config.rs` — config loading, serialization
- `src/notifications/mod.rs` — notification store operations

## Debugging

- Logs go to stderr (`eprintln!`)
- GTK debug: `G_MESSAGES_DEBUG=all`
- Stale PID detection runs every 5 seconds
