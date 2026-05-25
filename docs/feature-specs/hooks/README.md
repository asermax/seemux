# Agent Lifecycle Hook Integration

## Overview

End-to-end agent lifecycle integration: provider-neutral plugin hooks, standard JSON-RPC 2.0 Unix socket server, event parsing, session status mapping, unread notification badges, stale process cleanups, restart-resume orchestration, and programmatic command APIs.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [claude-code-integration](claude-code-integration.md) | Claude Code integration plugin mapping onto the generic contract | Current |
| [pi-dev-integration](pi-dev-integration.md) | Pi.dev extensible TypeScript integration mapping onto the generic contract | Current |

## Related Decisions

- ADR-003: Unix socket as IPC mechanism
- ADR-009: Claude hooks via plugin + socat
- DES-001: Background thread + mpsc + GTK poll pattern
