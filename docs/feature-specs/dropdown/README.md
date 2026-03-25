# Dropdown / Quake Mode

## Overview

Quake-style dropdown terminal with animated reveal, Wayland layer shell integration, dialog mode detection, global shortcut registration, and focus-loss recovery.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [quake-mode](quake-mode.md) | Dropdown window, layer shell, toplevel monitoring, global shortcuts, auto-hide | Current |

## Related Decisions

- ADR-004: Direct FFI to libgtk4-layer-shell
- ADR-005: Separate Wayland connection for toplevel monitoring
- ADR-010: ashpd for XDG Portal global shortcuts
- DES-008: Generation counter for animation/event cancellation
