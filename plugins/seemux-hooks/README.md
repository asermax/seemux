# seemux-hooks

Claude Code plugin that connects Claude sessions to the [seemux](https://github.com/asermax/seemux) terminal multiplexer.

When installed, Claude Code lifecycle events (session start, stop, notifications, tool use, etc.) are sent to seemux via Unix socket, enabling real-time status indicators and notification badges on your terminal tabs.

## Requirements

- [seemux](https://github.com/asermax/seemux) running as your terminal
- `socat` installed (used to send messages to the Unix socket)

## Install

```bash
# Add the seemux marketplace
claude /plugins marketplace add github asermax/seemux

# Install the plugin
claude /plugins install seemux-hooks
```

## How it works

The plugin registers hooks for key Claude Code events. Each hook runs a script that:

1. Checks for `$SEEMUX_SOCKET` and `$SEEMUX_SESSION_ID` env vars (set by seemux for each terminal)
2. Reads the event JSON from stdin
3. Sends it to seemux's Unix socket via `socat`

Seemux receives these events and updates the sidebar with status pills (Running, Needs Input, Completed, Error) and notification badges.

If you're not running inside a seemux terminal, the hooks exit silently with no side effects.
