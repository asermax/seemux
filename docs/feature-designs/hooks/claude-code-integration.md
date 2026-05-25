# Design: Claude Code Hook Integration

**Core Design**: [agent-lifecycle-integration.md](agent-lifecycle-integration.md)
**Feature Spec**: [../../feature-specs/hooks/claude-code-integration.md](../../feature-specs/hooks/claude-code-integration.md)
**Status**: Current

## Purpose

This document explains the Claude-specific design choices for Seemux's agent hook pipeline, specifically detailing the shell script bridge, the `socat` delivery mechanism, and the dependency-free native parsing fallback.

## Key Decisions

### Unix Socket with Shell Script Bridge

**Choice**: Use standard bash hooks + `socat` to deliver payloads to a local Unix socket.
**Why**:
- Claude Code's plugin extension model is strictly shell-command based; it executes scripts on the host filesystem when lifecycle events fire.
- `socat` is ubiquitous on Linux environments, making it a reliable transport medium.
- Exits immediately with code 0 if `$SEEMUX_SOCKET` is empty, completely avoiding blocking Claude when run outside of Seemux.

### Shell-Native Dependency-Free JSON Parsing Fallback

**Choice**: Use `grep`, `cut`, `sed`, and `tr` string manipulations in plain Bash to parse and merge JSON payloads when `jq` is not installed on the system.
**Why**:
- `jq` is highly robust for JSON operations but represents an external runtime dependency that might be missing on minimal Linux distributions.
- Since Claude Code hook payloads are predictably structured, native POSIX tool grep/cut regexes can reliably extract `.pid` and `.session_id` on start, and appending `,"session_id":"..."` inside `sed 's/}$//'` string merges allows joining Seemux metadata perfectly.
- Guarantees 100% fail-safe operation on any Linux host.
