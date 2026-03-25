# Application Shell

## Overview

The outermost layer of seemux: GTK application lifecycle, CLI parsing, window construction (normal + quake modes), subsystem wiring, GIO actions, overlay dialogs, and keyboard shortcuts.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [application-shell](application-shell.md) | Full application shell with window modes, actions, dialogs, keyboard handling | Current |

## Related Decisions

- ADR-001: GTK4 + VTE4 as UI/terminal framework
- ADR-002: Rc\<RefCell\<T\>\> for shared state
- DES-004: GIO action dispatch
- DES-007: Overlay dialog pattern
