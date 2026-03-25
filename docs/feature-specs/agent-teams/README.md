# Agent Teams Compatibility

## Overview

Tmux shim binary that intercepts tmux commands from Claude Code Agent Teams and translates them into seemux session/group operations.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [tmux-shim](tmux-shim.md) | Tmux command interception, pane map, session creation, fallthrough to real tmux | Current |

## Related Decisions

- ADR-003: Unix socket as IPC mechanism
- ADR-007: Tmux shim as separate binary
