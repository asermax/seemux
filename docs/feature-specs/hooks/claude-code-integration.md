# Claude Code Hook Integration Spec

**Core Specification**: [agent-lifecycle-integration.md](agent-lifecycle-integration.md)

## Overview

This specification describes Claude Code's specific mapping onto Seemux's generic agent lifecycle hook integration. It covers the Claude Code plugin structure, the registered hook events, and the `seemux-hook.sh` shell script translation layer.

## Requirements

| ID | Requirement |
|----|-------------|
| R1 | Register a native Claude Code plugin (`plugins/seemux-hooks/`) containing hook definitions for Claude's eight lifecycle events |
| R2 | Hook script `seemux-hook.sh` must be a no-op when `$SEEMUX_SOCKET` is unset, exiting with code 0 to avoid blocking Claude |
| R3 | Use `socat` to write translated NDJSON messages to Seemux's Unix socket |
| R4 | Map legacy Claude hook event names to canonical JSON-RPC 2.0 lifecycle methods in the shell script |
| R5 | Parse Claude Code's hook payload on stdin natively using `jq` to enrich the parameters of `agent.session.started` with `pid` and `session_id` |
| R6 | Provide a robust, dependency-free plain Bash fallback format when `jq` is not installed on the system, ensuring the hook never fails |

## Mappings

The `seemux-hook.sh` script maps legacy Claude hooks to canonical socket contract methods:

| Legacy Claude Hook | Canonical JSON-RPC Method | Fallback Behavior (No `jq`) |
|--------------------|---------------------------|-----------------------------|
| `session-start` | `agent.session.started` | Extracts `pid` and `session_id` using `grep`/`cut`/`tr` |
| `prompt-submit` | `agent.prompt.submitted` | Appends `session_id` using trailing `}` stripping |
| `pre-tool-use` | `agent.tool.pre_use` | Appends `session_id` using trailing `}` stripping |
| `post-tool-use` | `agent.tool.post_use` | Appends `session_id` using trailing `}` stripping |
| `post-tool-use-failure` | `agent.tool.failed` | Appends `session_id` using trailing `}` stripping |
| `notification` | `agent.attention.requested` | Appends `session_id` using trailing `}` stripping |
| `stop` | `agent.response.completed` | Appends `session_id` using trailing `}` stripping |
| `stop-failure` | `agent.response.failed` | Appends `session_id` using trailing `}` stripping |
| `session-end` | `agent.session.ended` | Appends `session_id` using trailing `}` stripping |
