# Claude Code Hook Integration

## Overview

End-to-end Claude Code integration: plugin hooks, Unix socket server, event parsing, session status mapping, notifications, stale PID detection, and programmatic command API.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [claude-code-integration](claude-code-integration.md) | Hook pipeline, notification store, command API, stale PID detection | Current |

## Related Decisions

- ADR-003: Unix socket as IPC mechanism
- ADR-009: Claude hooks via plugin + socat
- DES-001: Background thread + mpsc + GTK poll pattern
