# ADR-008: TOML for Config, JSON for Session State

**Status**: Accepted
**Date**: 2026-03-24
**Last Updated**: 2026-03-24

## Retrofit Note

This decision was inferred from existing code. Retrofit date: 2026-03-24

---

## Context

Seemux persists two categories of data: user preferences (font, color scheme, sidebar width) that change rarely and need human editability, and session state (tabs, split trees, groups) that changes frequently and contains recursive data structures.

## Decision

Use TOML at `~/.config/seemux/config.toml` for user configuration, and JSON at `~/.local/state/seemux/sessions.json` for session state. Both use atomic writes (tempfile + rename).

## Consequences

### Positive

- TOML is human-readable, supports comments, and is idiomatic for Rust apps
- JSON handles recursive split trees and Option types naturally via serde
- Separate files with separate XDG directories reflect different data roles
- serde provides unified serialization for both formats

### Negative

- Two serialization paths to maintain
- Flush logic must decide which files to write on each save

## Alternatives Considered

### JSON for both

- **Description**: Use JSON for config too
- **Why rejected**: Sacrifices human editability; no comment support

### TOML for both

- **Description**: Use TOML for session state too
- **Why rejected**: Recursive split trees awkward in TOML table syntax

### Single file

- **Description**: One file for both config and state
- **Why rejected**: Couples write frequencies (state changes constantly, config rarely)
