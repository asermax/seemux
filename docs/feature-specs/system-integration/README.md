# System Integration

## Overview

Desktop environment integration: system tray with notification badges via SNI protocol, and async git branch/GitHub PR detection for sidebar tab indicators.

## Sub-Capabilities

| Capability | Description | Status |
|------------|-------------|--------|
| [tray-and-git](tray-and-git.md) | System tray icon, badge rendering, git branch detection, GitHub PR detection | Current |

## Related Decisions

- ADR-006: ksni for system tray (SNI protocol)
- DES-001: Background thread + mpsc + GTK poll pattern
