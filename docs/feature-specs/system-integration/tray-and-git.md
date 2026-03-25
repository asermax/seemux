# System Tray and Git/PR Detection

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/tray.rs`, `src/git.rs`.
Retrofit date: 2026-03-24

---

## Overview

Two system integration capabilities: a **system tray** providing persistent desktop presence via the SNI protocol with notification badge rendering using a built-in bitmap font, and **async Git/PR detection** that runs `git` and `gh` CLI commands on background threads to discover branch names and associated pull requests for sidebar tab indicators.

## User Stories

- As a desktop user, I want a tray icon showing unread notification counts so I can see whether sessions need attention without switching to the Seemux window.
- As a user who has hidden Seemux, I want to click the tray icon to bring it back.
- As a user in quake mode, I want tray click to toggle the dropdown.
- As a developer, I want each tab to show the current git branch so I can orient myself.
- As a developer, I want each tab to show the associated open PR number and link.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | System tray icon via SNI/ksni reflecting unread notification state with window activation on click |
| R1 | Tray icon at four resolutions (128, 48, 32, 22 px); smaller variants derived via box-filter downscaling |
| R2 | Colored circular badge with count rendered as white digits using built-in 4x6 bitmap font |
| R3 | Counts above 9 displayed as "9+" |
| R4 | Badge color from active theme's accent color |
| R5 | Badge circle occupies larger fraction at smaller sizes (50% at <=24px, 42% at <=48px, 38% at >48px) |
| R6 | Left-click in normal mode sends activate-window event; in quake mode sends toggle-dropdown |
| R7 | Context menu with "Quit" item sending quit event |
| R8 | Tooltip shows "Seemux" when idle, "Seemux -- N unread" with notifications |
| R9 | Tray can be disabled via `tray_enabled` config option |
| R10 | Badge icons cached, only recomputed when count changes |
| R11 | SNI status: Active (no unread) / NeedsAttention (unread > 0) |
| R12 | Git branch detection via `git rev-parse --abbrev-ref HEAD` on background thread |
| R13 | GitHub PR detection via `gh pr list --head <branch>` on background thread |
| R14 | Both async ops use mpsc + 50ms GTK polling to avoid blocking |
| R15 | PR detection chained after branch detection; clears PR when no branch |

## Behaviors

### Tray Initialization

**Acceptance Criteria**:
- Given tray enabled in normal mode, when window is built, then tray appears with "seemux" icon, Active status, tooltip "Seemux"
- Given tray enabled in quake mode, then tray appears with toggle-dropdown behavior
- Given tray disabled, then no tray is registered and TrayHandle is a no-op

### Badge Rendering

**Acceptance Criteria**:
- Given 0 unread, then no badge, theme icon name used, status Active
- Given 1-9 unread, then badge shows the digit count in white on accent-colored circle
- Given 10+ unread, then badge shows "9+"
- Given count changes from 3 to 3, then cached icons are NOT recomputed
- Given count changes from 3 to 5, then icons are recomputed

### Tray Interaction

**Acceptance Criteria**:
- Given normal mode, when left-clicked, then activate-window event sent via socket
- Given quake mode, when left-clicked, then toggle-dropdown event sent via socket
- Given context menu "Quit" clicked, then quit event sent
- Given socket unavailable, then send silently fails

### Git Branch Detection

**Acceptance Criteria**:
- Given a CWD inside a git repo, when detect_branch_async is called, then branch name is delivered to callback on GTK main thread
- Given a CWD not in a git repo, then None is delivered
- Given git is not installed, then None is delivered

### PR Detection

**Acceptance Criteria**:
- Given a detected branch with an open PR, when detect_pr_async is called, then PrInfo (number, url) is delivered
- Given a branch with no open PR, then None is delivered
- Given gh is not installed or unauthenticated, then None is delivered
- Given no branch detected, then PR detection is skipped and indicator is cleared

### Async Execution

**Acceptance Criteria**:
- Given an async operation dispatched via run_async, when the thread is working, then the GTK timer polls every 50ms without blocking
- Given the thread completes, when the next poll occurs, then the result is delivered and the timer is removed
- Given the thread panics, when the channel disconnects, then the timer removes itself without callback
