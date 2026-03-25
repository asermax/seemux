# Persistence and Configuration

## Overview

TOML configuration, JSON session state persistence, debounced dirty-flag saving, atomic writes, shared app state, and XDG runtime directory management.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [configuration-and-state](configuration-and-state.md) | Config loading, state persistence, debounced saving, runtime dirs, tmux shim deployment | Current |

## Related Decisions

- ADR-008: TOML for config, JSON for session state
- DES-003: Debounced persistence with dirty flag
- DES-005: Atomic file writes
