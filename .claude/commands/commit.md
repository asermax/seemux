---
description: Commits changes with automatic semantic versioning for the seemux app and plugins
allowed-tools: Read, Edit, Bash(git diff:*), Bash(git status:*), Bash(git add:*), Bash(git commit:*), Bash(jq:*), Bash(mv:*), Bash(sed:*), AskUserQuestion
---

# Commit with Version Bump

Creates conventional commits with automatic semantic versioning for the seemux app and its plugins.

## Usage

```bash
/commit
```

## Overview

This command automates the commit process for the seemux project by:
1. Analyzing all changed files (staged and unstaged)
2. Grouping related changes into logical commits
3. Determining affected scopes (seemux app, seemux-hooks plugin)
4. Auto-proposing semantic version bumps based on change significance
5. Creating conventional commits with version updates

## Version Files

| Scope | File Path | Version Field |
|-------|-----------|---------------|
| seemux (app) | `Cargo.toml` | `version` field in `[package]` |
| seemux-hooks | `plugins/seemux-hooks/.claude-plugin/plugin.json` | `.version` |
| marketplace sync | `.claude-plugin/marketplace.json` | `.plugins[].version` (must match plugin.json) |

## Semantic Versioning Rules

### App Versioning (seemux)

The app version in `Cargo.toml` tracks the terminal multiplexer itself.

| Version | Guideline | Examples |
|---------|-----------|----------|
| **Major** | Breaking changes to user-facing behavior | Config format change, removed feature, CLI breaking change |
| **Minor** | New functionality | New keyboard shortcut, new UI feature, new config option |
| **Patch** | Bug fixes, small improvements | Fix focus issue, fix rendering glitch, small UX tweak |

### Plugin Versioning (seemux-hooks)

Individual plugin versions track their own feature development:

#### Major (Breaking Changes)
- Removing a hook entirely
- Changing hook event format in a breaking way
- Removing significant functionality

#### Minor (Features)
- Adding a new hook
- Adding new event types
- Significant improvements to hook behavior

#### Patch (Fixes)
- Small script fixes
- Documentation updates
- Bug fixes in hook logic
- Minor clarifications

## Process

### 1. Gather Changes

Run git commands to understand the current state:

```bash
git status --short
git diff --cached --name-only  # staged files
git diff --name-only           # unstaged files
```

Parse the output to get:
- Modified files (M)
- Added files (A)
- Deleted files (D)
- Renamed files (R)

### 2. Group Related Changes

Use these heuristics to cluster files into logical commit groups:

#### Auto-grouping Rules

| Pattern | Grouping Logic |
|---------|----------------|
| `src/**` | Group by feature area (related source files together) |
| `plugins/seemux-hooks/hooks/*` | Group hooks together unless clearly unrelated |
| `plugins/seemux-hooks/scripts/*` | Group with related hook changes |
| `plugins/seemux-hooks/README.md` | Group with other seemux-hooks changes |
| `.claude/commands/*.md` | Each command file = separate commit |
| `.claude-plugin/marketplace.json` | Separate commit unless part of plugin add/remove |
| `resources/**` | Group with related source changes if applicable |
| `Cargo.toml` | Group with related source changes |
| Root config files | Separate commits unless clearly related |

#### Example Groupings

**Scenario 1**: Changed `src/session/manager.rs` and `src/session/mod.rs`
→ **Single commit**: Related changes (same module)

**Scenario 2**: Changed `src/app.rs` and `plugins/seemux-hooks/scripts/seemux-hook.sh`
→ **Two commits**: Different scopes (app vs plugin)

**Scenario 3**: Changed `plugins/seemux-hooks/hooks/hooks.json` and `plugins/seemux-hooks/scripts/seemux-hook.sh`
→ **Single commit**: Related plugin changes

### 3. Present Proposed Groups

Display the proposed commit groupings to the user:

```
Proposed commit groups:
1. [seemux/feat] Add context menu for file links
   - src/terminal/vte_terminal.rs
   - src/app.rs

2. [seemux-hooks/fix] Fix hook event format
   - plugins/seemux-hooks/scripts/seemux-hook.sh

Would you like to adjust these groupings?
```

Use AskUserQuestion to offer options:
- Proceed with proposed groups
- Merge groups (combine into fewer commits)
- Split groups (separate into more commits)
- Exclude files (don't commit yet)

### 4. Process Each Commit Group

For each commit group, execute these steps:

#### a. Determine Affected Scopes

Map file paths to scopes:

| Path Pattern | Scope |
|--------------|-------|
| `src/**` | `seemux` (app) |
| `Cargo.toml` | `seemux` (app) |
| `build.rs` | `seemux` (app) |
| `resources/**` | `seemux` (app) |
| `plugins/seemux-hooks/**` | `seemux-hooks` (plugin) |
| `.claude-plugin/**` | `marketplace` |
| `.claude/commands/**` | `marketplace` |
| Root files (`README.md`, `CLAUDE.md`, etc.) | `marketplace` |

#### b. Analyze Change Significance

For each affected scope, determine the change type:

**For seemux (app)**:

- **Check for Major changes**:
  - Config format changes that break existing configs → Breaking (major)
  - CLI argument changes → Breaking (major)
  - Removed features → Breaking (major)

- **Check for Minor changes**:
  - New features (keyboard shortcuts, UI elements, config options) → Feature (minor)
  - New modules added → Feature (minor)

- **Default to Patch**:
  - Bug fixes, small improvements → Patch
  - Refactoring → Patch
  - Style/CSS changes → Patch

**For seemux-hooks (plugin)**:

- **Check for Major changes**:
  - Hooks removed or renamed → Breaking (major)
  - Event format changes → Breaking (major)

- **Check for Minor changes**:
  - New hooks added → Feature (minor)
  - New event types → Feature (minor)

- **Default to Patch**:
  - Script fixes, documentation → Patch

#### c. Read Current Versions

Parse version files:

```bash
grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'
jq -r '.version' plugins/seemux-hooks/.claude-plugin/plugin.json
```

#### d. Calculate New Versions

For each affected scope:

```
Current: X.Y.Z

Major bump: (X+1).0.0
Minor bump: X.(Y+1).0
Patch bump: X.Y.(Z+1)
```

#### e. Propose Version Bump

Present the analysis to the user:

```
Commit: Fix hook event format
Affected scopes:
  - seemux-hooks plugin

Changes:
  - Fixed event JSON format in hook script

Proposed version bumps:
- seemux-hooks: 0.1.0 → 0.1.1 (patch: bug fix)

Proposed commit message:
fix(seemux-hooks): correct hook event JSON format

Fixed malformed JSON in hook event payload that caused
status updates to fail.

Version bump: seemux-hooks 0.1.0 → 0.1.1
```

**Include brief reasoning** for each version bump in parentheses so the user can validate the judgment call.

Use AskUserQuestion to confirm or adjust. **Important**: Offer version type alternatives based on what was proposed:

**If proposing Major bump:**
- Confirm and proceed
- Downgrade to minor bump
- Downgrade to patch bump

**If proposing Minor bump:**
- Confirm and proceed
- Downgrade to patch bump

**If proposing Patch bump:**
- Confirm and proceed
- Upgrade to minor bump (if it's actually a feature)

#### f. Update Version Files

If version bump approved, update the appropriate file(s):

**For seemux (app)** — update `Cargo.toml`:
```bash
sed -i 's/^version = ".*"/version = "X.Y.Z"/' Cargo.toml
```

**For seemux-hooks** — update both the plugin.json AND sync the version in marketplace.json:
```bash
jq '.version = "X.Y.Z"' plugins/seemux-hooks/.claude-plugin/plugin.json > /tmp/seemux-hooks.json
mv /tmp/seemux-hooks.json plugins/seemux-hooks/.claude-plugin/plugin.json

jq '(.plugins[] | select(.name == "seemux-hooks")).version = "X.Y.Z"' .claude-plugin/marketplace.json > /tmp/marketplace.json
mv /tmp/marketplace.json .claude-plugin/marketplace.json
```

#### g. Stage Files

Stage the changed files plus any updated version files:

```bash
git add <changed-file-1> <changed-file-2> ... <version-file>
```

#### h. Create Commit

Create the conventional commit using the approved message:

```bash
git commit -m "$(cat <<'EOF'
<type>(<scope>): <description>

<body explaining what changed and why>

Version bump: <scope> X.Y.Z → X.Y.Z

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

### 5. Repeat for Each Group

Continue processing each commit group until all approved groups are committed.

## Commit Message Format

Each commit follows conventional commits specification:

```
<type>(<scope>): <short description>

<longer description explaining the change>

Version bump: <scope-name> X.Y.Z → X.Y.Z

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
```

### Type Mapping

| Change Significance | Commit Type | Notes |
|-------------------|-------------|--------|
| Major (breaking) | Include `BREAKING CHANGE:` in commit body | Breaking changes must be clearly marked |
| Minor (feature) | `feat` | New functionality, backward compatible |
| Patch (fix) | `fix`, `docs`, `refactor`, `style`, `chore` | Bug fixes and improvements |

### Scope Values

| Scope | Description |
|-------|-------------|
| `seemux` | Changes to the terminal multiplexer app |
| `seemux-hooks` | Changes to the seemux-hooks plugin |
| `marketplace` | Changes to marketplace config or root commands |

### Examples

**Patch commit** (plugin fix):
```
fix(seemux-hooks): correct hook event JSON format

Fixed malformed JSON in hook event payload that caused
status updates to fail silently.

Version bump: seemux-hooks 0.1.0 → 0.1.1

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
```

**Minor commit** (app feature):
```
feat(seemux): add context menu for file links

Opens file:// links in $EDITOR via right-click context menu
on terminal output.

Version bump: seemux 0.1.0 → 0.2.0

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
```

**Major commit** (breaking change):
```
feat(seemux-hooks): restructure hook event format

BREAKING CHANGE: Hook events now use a nested payload structure.
Existing Claude Code hook configurations will need to be updated.

Version bump: seemux-hooks 0.1.0 → 1.0.0

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
```

## Error Handling

### No Changes Detected
If `git status` shows no changes:
- Inform user: "No changes to commit. Working directory is clean."
- Exit gracefully

### Merge Conflicts
If files have merge conflicts:
- List conflicted files
- Instruct user to resolve conflicts first
- Exit without committing

### Invalid Version Format
If JSON files have malformed version strings:
- Report the issue
- Ask user to fix manually
- Exit without committing

### Version Sync Check
Before committing, verify that the plugin version in `marketplace.json` matches `plugin.json`. If they diverge, fix the sync before committing.

### Git Command Failures
If any git command fails:
- Show the error message
- Explain what went wrong
- Provide guidance on how to recover

## Notes

- **Always use conventional commits format** for consistency
- **Version bumps are per-scope** (app and plugins version independently)
- **Multiple commits for unrelated changes** keeps history clean and focused
- **User confirmation required** before any version bump or commit
- **Git must be in clean state** (no unresolved conflicts)
- **Staged and unstaged changes** are both analyzed and can be included
- **File grouping is intelligent** but user has final say on what goes together
- **Plugin versions must stay in sync** between `plugin.json` and `marketplace.json`
