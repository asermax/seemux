# Sidebar Navigation and Session Organization

<!-- This spec describes the current system capability. Updated through delta reconciliation. -->

## Retrofit Note

This spec was created from existing code at `src/sidebar/mod.rs`, `src/sidebar/tab_row.rs`, `src/sidebar/tab_group.rs`, `src/sidebar/collapsed_bar.rs`.
Retrofit date: 2026-03-24

---

## Overview

The sidebar is Seemux's primary navigation surface. It presents terminal sessions as interactive tab rows organized into a default section and named collapsible groups. Each tab row displays title, working directory, git branch, PR link, Claude Code status pill, notification preview, unread badge, and close button. The sidebar supports expanded (full tab rows) and collapsed (dot-bar) modes, drag-and-drop reordering of tabs and groups, peek behavior for collapsed groups, and Alt-key index overlays for keyboard navigation.

## User Stories

- As a user, I want a sidebar that organizes sessions into named groups with rich tab metadata so I can quickly navigate and understand session state at a glance.
- As a power user, I want to collapse the sidebar into a minimal dot bar and collapse individual groups so I can maximize terminal space without losing awareness.
- As a user managing many sessions, I want to reorder tabs and groups via drag-and-drop and move tabs between groups.

## Requirements

| ID | Requirement |
|----|-------------|
| R0 | Display terminal sessions as navigable tab rows organized into a default section and named groups |
| R1 | Each tab row displays: title, subtitle/path, folder icon, git branch, PR link, status pill, notification preview, unread badge, close button, index overlay |
| R2 | Support expanded mode (full tab rows) and collapsed dot-bar mode (colored circles) |
| R3 | Named groups have a collapsible header with toggle, name label, add-tab button, and context menu |
| R4 | Collapsed groups use "peek" behavior to keep important tabs visible (active, running, or with unread badges) |
| R5 | Tabs are reorderable within and between groups via drag-and-drop |
| R6 | Named groups are reorderable via drag-and-drop on headers |
| R7 | Tab rows and group headers expose right-click context menus |
| R8 | When Alt is held, visible tabs show numeric index overlays (1-9) |
| R9 | Collapsed dot bar renders status via color-coded Cairo circles with accent ring for active session |
| R10 | Deleting a group moves its tabs back to the default section |
| R11 | Sidebar remembers expanded width when collapsing and restores on expand |
| R12 | Dropping a tab onto a collapsed group auto-expands that group |
| R13 | Default section has "Add tab" button; "New Group" button below groups |
| R14 | Status pill driven by session status: hidden for Idle/Exited, visible with CSS class for Running/NeedsInput/Completed/Error |
| R15 | Groups show "No tabs yet" placeholder when empty |

## Behaviors

### Tab Row Display

**Acceptance Criteria**:
- Given a tab row, when CWD is updated, then title shows folder name, folder icon is visible, subtitle shows display path
- Given a tab row, when a non-empty branch is set, then branch label shows icon + name; when cleared, branch and PR labels hide
- Given a tab row, when PR number/URL are set, then PR label shows clickable "PR#N" link
- Given a tab row, when status is Running/NeedsInput/Completed/Error, then status pill is visible with CSS class; when Idle/Exited, pill is hidden
- Given a tab row, when badge count > 0, then badge is visible; when 0, badge is hidden
- Given a tab row, when notification preview is set, then preview label shows (ellipsized at 25 chars); when cleared, hidden
- Given a tab row, when set as active, then it gets "active" CSS class and all others lose it
- Given a tab row set as active that is not fully visible in the sidebar scroll viewport, then the sidebar scrolls the minimum amount to reveal it (top-edge aligned when above, bottom-edge aligned when below)

### Tab Index Overlay

**Acceptance Criteria**:
- Given Alt is held, then visible tabs 1-9 show index labels and hide close buttons; tabs beyond 9 show no index
- Given Alt is released, then index labels hide and close buttons restore
- Given a collapsed group, then only peeked tabs receive indices

### Sidebar Collapse

**Acceptance Criteria**:
- Given the sidebar is expanded, when collapsed, then the dot bar replaces the tab list and width is set to 24px
- Given the sidebar is collapsed, when expanded, then the tab list restores and width returns to the remembered expanded width
- Given the sidebar is collapsed, then `effective_sidebar_width` returns the remembered expanded width

### Collapsed Dot Bar

**Acceptance Criteria**:
- Given collapsed mode, when rebuilt, then one circle per visible session appears in sidebar order
- Given a dot, then its color matches the session status from the theme
- Given the active session dot, then an accent ring is drawn around it
- Given a status change, then the dot redraws without full rebuild
- Given a dot is clicked, then the on_dot_click callback fires

### Group Collapse and Peek

**Acceptance Criteria**:
- Given an expanded group, when header is clicked, then it collapses and tabs satisfying "should_peek" (active, has badge, or running) peek out
- Given a collapsed group, when header is clicked, then it expands and all peek flags clear
- Given a collapsed group, when a session transitions to Running, then that tab peeks
- Given a peeking tab whose status returns to Idle with no badge and not active, then peek is removed

### Tab Drag-and-Drop

**Acceptance Criteria**:
- Given a tab drag starts, then the row gets "dragging" CSS class and shared dragging_id is set
- Given a tab is dragged over another tab (not itself), then the target shows a "drop-after" indicator
- Given a tab is dropped in a different group, then it moves to the target group at the insertion point
- Given a tab is dropped onto a collapsed group, then that group auto-expands
- Given a tab is dropped onto a group header, then it inserts at position 0

### Group Drag-and-Drop

**Acceptance Criteria**:
- Given a group drag starts, then the container gets "dragging" CSS class
- Given a group is dragged over another group, then the target shows "drop-after-group" indicator
- Given a group is dropped, then the widget and internal list reorder
- Given a group is dropped onto the "Groups" header, then it moves to first position

### Context Menus

**Acceptance Criteria**:
- Given a right-click on a tab row, then a popover with "Close" and "Close Others" appears
- Given a right-click on a group header, then a popover with "Rename Group" and "Delete Group" appears

### Group Lifecycle

**Acceptance Criteria**:
- Given a group with tabs is deleted, then all tabs move to the default section first
- Given a group is renamed, then both internal entry and display label update
- Given a named group has no tabs, then a "No tabs yet" placeholder is shown
