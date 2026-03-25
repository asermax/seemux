# Feature Specifications

System capabilities organized by domain.

## Capability Domains

| Domain | Description |
|--------|-------------|
| [app-shell](app-shell/) | Application lifecycle, window modes, actions, dialogs, keyboard shortcuts |
| [session](session/) | Session lifecycle, split panes, tab switching, Claude integration |
| [terminal](terminal/) | VTE4 terminal emulation, split pane tree, URL detection, scroll guard |
| [sidebar](sidebar/) | Tab navigation, groups, drag-and-drop, collapsed dot bar, peek behavior |
| [hooks](hooks/) | Claude Code hook integration, notifications, socket command API |
| [dropdown](dropdown/) | Quake-style dropdown terminal, layer shell, dialog mode, global shortcuts |
| [persistence](persistence/) | TOML config, JSON session state, debounced saving, runtime directories |
| [theming](theming/) | Color schemes, runtime CSS generation |
| [system-integration](system-integration/) | System tray, git branch and PR detection |
| [agent-teams](agent-teams/) | Tmux shim for Claude Code Agent Teams compatibility |
