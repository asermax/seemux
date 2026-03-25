# Session Management

## Overview

Central orchestration layer: session lifecycle, split pane management, tab switching algorithms, VTE signal wiring, Claude PID/status tracking, and session persistence.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [session-management](session-management.md) | Full session lifecycle, splits, switching, persistence, Claude integration | Current |

## Related Decisions

- ADR-002: Rc\<RefCell\<T\>\> for shared state
- DES-002: Callback-based event wiring
- DES-006: Deferred shell spawning for collapsed groups
