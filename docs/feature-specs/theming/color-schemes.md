# Color Schemes

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/theme.rs`.
Retrofit date: 2026-03-24

---

## Overview

Seemux renders with a consistent, visually cohesive color scheme across all UI surfaces. The theming system defines color values for terminal emulator colors, UI chrome colors, and session status colors. Two built-in schemes ship with seemux (Catppuccin Mocha and Dracula). At startup, the configured scheme is resolved and a complete GTK4 CSS stylesheet is generated at runtime, styling every widget in both the main window and dropdown/quake modes.

## User Stories

- As a user, I want the terminal multiplexer to render with a consistent color scheme across all UI surfaces so that the interface is readable and aesthetically pleasing.
- As a user, I want to choose between color schemes so that I can pick the one that suits my preference.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Apply a user-configurable color scheme that consistently themes all UI surfaces: terminal, sidebar, tabs, status indicators, overlays, and system tray |
| R1 | Each scheme defines terminal colors: foreground, background, and 16-color palette |
| R2 | Each scheme defines UI chrome colors: window background, sidebar background, accent, text hierarchy (primary, secondary, muted), separator, hover, and active surfaces |
| R3 | Each scheme defines five session status colors: running, needs-input, error, completed, and idle |
| R4 | Ship with two built-in schemes: Catppuccin Mocha (default) and Dracula |
| R5 | Scheme selection resolved from `color_scheme` field in `config.toml` at startup |
| R6 | Unknown scheme names silently fall back to Catppuccin Mocha |
| R7 | A complete GTK4 CSS stylesheet is generated at runtime from the resolved scheme |
| R8 | Generated CSS styles all major widget classes: tabs, groups, status pills, badges, overlays, context menus, drag-and-drop feedback, dropdown border, and collapsed states |
| R9 | VTE terminals receive scheme colors via `set_colors` API, independent of CSS |
| R10 | Collapsed sidebar precomputes status colors as floating-point RGB for Cairo draw calls |
| R11 | System tray receives scheme accent color for badge rendering |
| R12 | CSS transitions defined for interactive elements: tab hover (150ms), active indicator (200ms), close button opacity (150ms), DnD margin shifts (150ms) |

## Behaviors

### Scheme Resolution

**Acceptance Criteria**:
- Given a config with `color_scheme = "catppuccin-mocha"`, when the app starts, then the Catppuccin Mocha scheme is applied
- Given a config with `color_scheme = "dracula"`, when the app starts, then the Dracula scheme is applied
- Given a config with `color_scheme = "nonexistent"`, when the app starts, then it silently falls back to Catppuccin Mocha
- Given no `color_scheme` field, when config loads with defaults, then it defaults to `"catppuccin-mocha"`

### CSS Generation

**Acceptance Criteria**:
- Given a resolved color scheme, when `generate_css` is called, then the CSS contains a comment identifying the scheme name
- Given any scheme, when the CSS is loaded into a `CssProvider`, then it is registered at `STYLE_PROVIDER_PRIORITY_APPLICATION`, overriding GTK defaults

### Terminal Colors

**Acceptance Criteria**:
- Given a VTE terminal is created, when colors are applied, then foreground, background, and 16-color palette are set from the scheme
- Given a scheme with valid hex color strings, when parsed via `gdk::RGBA::parse`, then all 18 colors parse successfully

### UI Surface Styling

**Acceptance Criteria**:
- Given the CSS is active, when a tab row is hovered, then its background transitions to `surface_hover` over 150ms
- Given the CSS is active, when a tab row is the active session, then its background is `surface_active` with an accent-colored active indicator
- Given the CSS is active, when a session has status "error", then `.status-pill--error` renders text in the scheme's `status_error` color
- Given the CSS is active, when a session has unread notifications, then the badge uses the scheme's accent color
- Given the CSS is active, when a tab's close button is not hovered, then it has opacity 0, revealing at 0.5 on row hover and 1.0 on direct hover

### Collapsed Sidebar

**Acceptance Criteria**:
- Given the sidebar is collapsed, when session dots are drawn via Cairo, then each dot's fill color matches its session status using precomputed RGB values
- Given a session is active, when its dot is drawn, then an accent-colored ring surrounds the dot

### Cross-Mode Consistency

**Acceptance Criteria**:
- Given the app is in dropdown/quake mode, when the dropdown window is built, then the same scheme is applied and the dropdown border uses the accent color
- Given the system tray is enabled, when it is set up, then the accent color is passed for badge rendering
