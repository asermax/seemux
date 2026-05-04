# Draft Design: Session Management

## Retrofit Note

Inferred from existing code at:
- `src/session/mod.rs` -- Session data model and status enum
- `src/session/manager.rs` -- SessionManager orchestration layer

## Problem Context

Seemux needs a central abstraction that ties together a terminal (or tree of terminals), a sidebar tab, an optional Claude Code agent, and lifecycle metadata into a coherent unit the user can create, switch between, organize, and persist across restarts.

- **Constraints**: Single-threaded GTK4 event loop; all shared state must be `Rc<RefCell<T>>`. Signals from VTE terminals (title change, CWD change, child exit, bell) arrive asynchronously and must update UI without blocking. Collapsed session groups should not consume resources by spawning shells until expanded.
- **Interactions**: VTE4 terminal widget (shell process), GTK4 Stack (page switching), Sidebar (tab display and ordering), SplitView/SplitTree (pane layout), NotificationStore (unread counts), HookServer (Claude Code lifecycle events), git/gh CLI (branch and PR detection), filesystem (config TOML, session state JSON).
- **Scope**: Covers session CRUD, split pane management, VTE signal wiring, tab switching and navigation, group membership, persistence, deferred spawning, Claude integration hooks, and non-Claude command status tracking. Does NOT cover the hook server protocol, sidebar widget internals, or terminal configuration.

## Design Overview

The session domain is structured as two complementary pieces:

1. **Session** (data model) -- A plain struct representing the identity and metadata of a session: UUID, title, lifecycle status, optional Claude PID/session ID, CWD, group membership, and a transient `pending_resume_id` for post-restart Claude session recovery.

2. **SessionManager** (orchestrator) -- Owns the collection of sessions, their split views, the GTK stack, and the sidebar reference. It coordinates all session operations: creation, destruction, switching, splitting, navigation, signal wiring, persistence, and deferred spawning. It acts as the single mediator between VTE terminal events and the UI layer.

The manager exposes two API styles:
- **Instance methods** (`&mut self`) for operations that only need mutable access to the manager (create, destroy, switch, navigate).
- **Static methods** (`Rc<RefCell<Self>>`) for operations that need to wire GTK signal handlers with closures that capture a reference back to the manager (split, child-exited, focus tracking, bell, status).

## Modeling

### Entity Relationships

```
SessionManager (1)
├── sessions: Vec<Session>           -- ordered list of all sessions
├── split_views: HashMap<id, SplitView>  -- one split tree per session
├── active_id: Option<String>        -- currently focused session
├── session_cwds: HashMap<pane_id, path>  -- per-pane CWD tracking
├── browser_panes: RefCell<HashMap<pane_id, BrowserPaneState>>  -- per-pane browser state
├── next_debug_port: Cell<u16>       -- counter starting at 19300
├── bell_timestamps: HashMap<session_id, unix_ts>  -- debounce state
├── stack: GTK Stack                 -- page container for terminal widgets
├── sidebar: Sidebar                 -- tab list and group UI
├── config: Config                   -- terminal and app settings
├── notification_store: NotificationStore  -- unread count per session
├── on_empty: Callback               -- fires when last session destroyed
├── on_state_changed: Callback       -- fires on any state mutation
└── on_browser_error: Callback<String>  -- fires when browser pane crashes early

Session
├── id: String (UUID)
├── title: String
├── status: SessionStatus {Idle, Running, NeedsInput, Completed, Error, Exited}
├── session_type: SessionType {Shell, Browser}  -- set at creation, reflects origin
├── claude_pid: Option<u32>
├── claude_session_id: Option<String>
├── pending_resume_id: Option<String>  -- transient, serde-skipped
├── created_at: i64 (unix timestamp)
├── cwd: Option<String>
└── group_id: String ("default" or named group ID)

SplitView (1 per session)
├── tree: SplitTree {Leaf(pane_id) | Split{orientation, first, second}}
├── panes: HashMap<pane_id, VteTerminal>  -- flat terminal storage
└── focused_pane_id: String

BrowserPaneState (per-pane, in SessionManager)
├── url: String
├── page_title: Option<String>
├── debug_port: u16
├── poll_timer: Option<glib::SourceId>  -- GTK timer for try_recv
├── poll_rx: Option<mpsc::Receiver<PollResult>>  -- from background thread
├── stop_poll: Option<Arc<AtomicBool>>  -- signals thread termination
└── created_at: Instant  -- used for crash detection (< 2s = crash)

SessionState (persistence)
├── sessions: Vec<SavedSession>  -- ordered by sidebar position
├── groups: Vec<SavedGroup>      -- id, name, collapsed flag
└── active_session_index: Option<usize>
```

### State Lifecycle

```
Session Status Flow:

  [Created] --> Idle
                 |
                 +--> Running (command detected via VTE title, 3s delay for non-Claude)
                 |      |
                 |      +--> Idle (shell prompt returns)
                 |      +--> NeedsInput (Claude hook)
                 |      +--> Completed (Claude hook)
                 |      +--> Error (Claude hook)
                 |
                 +--> Exited (child process terminated)
```

For non-Claude sessions, the `Running` status is driven entirely by VTE title heuristics with a 3-second debounce delay. For Claude sessions, status is driven by hook events and the VTE-based status tracking is explicitly bypassed.

## Data Flow

### 1. Session Creation

1. **Entry**: `create_session_inner` receives optional title, CWD, group ID, and spawn action (shell or command).
2. **Process**: Generates UUID, creates `Session` struct, builds environment variables (`SEEMUX_SOCKET`, `SEEMUX_SESSION_ID`, optionally `PATH` with bin dir), creates `VteTerminal` with config, spawns shell/command if the GTK stack is realized.
3. **Signal wiring**: Connects VTE title-changed and CWD-changed handlers to update sidebar tab title, folder name, subtitle path, and trigger async git branch + PR detection.
4. **Registration**: Adds the split view widget to the GTK stack, adds a tab to the sidebar, switches focus to the new session, notifies state change.
5. **Output**: Returns the session UUID.

### 2. VTE Signal Processing

**Title changed** (two independent handlers per pane):

- *Tab title handler*: If the title matches shell prompt pattern (`user@host:/path`), reverts tab to folder name. If the previous title was a git/gh command, schedules a 2-second debounced re-detection of branch and PR. Otherwise, updates the tab with the command name.
- *Status handler* (non-Claude only): Detects command start (non-shell title) and schedules a 3-second delayed "Running" pill. On shell title return, cancels the pending badge or hides it and emits a completion notification for background tabs.

**CWD changed**: Updates the shared `session_cwds` map, updates sidebar folder/subtitle, cancels any pending git re-detection, triggers fresh branch + PR detection. Skips if CWD is unchanged.

**Child exited**: Deferred via `glib::idle_add_local_once` to avoid re-entrant borrow. Closes the pane; if it was the last pane, destroys the session.

**Bell**: Debounced to 1 per 2 seconds per session. Ignored for the active session. Creates a notification in the store.

### 3. Session Destruction

1. **Entry**: `destroy_session` receives session ID.
2. **Process**: Captures group siblings (for focus redirection) before removal. Cleans up pane CWDs, bell timestamps, GTK stack child, sidebar tab, split view, and session from the vec.
3. **Focus redirection**: If the destroyed session was active, finds the best next focus: previous sibling in group, next sibling, first visible tab, or first tab in any group.
4. **Output**: Fires `on_empty` if no sessions remain; fires `on_state_changed` otherwise.

### 4. Split Pane Operations

1. **Split**: Gets focused terminal's CWD, delegates to `SplitView::split` to modify the tree and create a new terminal, spawns a shell in the new pane with inherited CWD, rebuilds the widget tree in the GTK stack, wires all signal handlers on the new terminal.
2. **Close pane**: Delegates to `SplitView::close_focused_pane`. If last pane, signals the caller to destroy the session. Otherwise, rebuilds widget tree and re-focuses.

### 5. Navigation

- **Tab switching**: Circular index arithmetic over visible session IDs from the sidebar.
- **Group switching**: Circular over visible group IDs, activates first session of target group.
- **Pane navigation**: Delegates to `SplitView::navigate` with a `Direction` enum.
- **Notification/status jumping**: `find_adjacent_matching` iterates circularly with a predicate (unread count > 0, non-idle `SessionStatus`, or any pane with a running command detected via VTE title heuristics).
- **Index switching**: `Ctrl+1..9` maps to visible session index.

### 6. Persistence

- **Save**: Iterates sessions in sidebar-ordered sequence. For each, serializes title, split tree (via `SplitView::to_saved` with pane CWDs), group ID, and Claude session ID. Also saves group metadata (id, name, collapsed) and active session index.
- **Restore**: `restore_session_with_splits` recreates session with split tree from `SavedSplitNode`, wires VTE signals, stores pane CWDs, registers session. Shell spawning is deferred until `spawn_deferred` (for non-collapsed groups) or `switch_to` (for individual sessions).

### 7. Error Paths

- VTE URI parsing failure: branch/PR cleared, CWD update skipped.
- `try_borrow` failure on `Rc<RefCell<SessionManager>>`: signal handler silently returns (prevents panic from re-entrant access).
- Config file parse failure: falls back to defaults, logs to stderr.
- Session state parse failure: starts fresh with no sessions, logs to stderr.

### 8. Browser Session Creation

1. **Entry**: `create_browser_session(url)` or `split_with_browser(self_ref, url)`.
2. **Pre-check**: Carbonyl availability checked via cached `which carbonyl` result. If not found, error overlay shown (DES-007) and no session created.
3. **Process**: Creates `Session` with `SessionType::Browser`, creates `SplitView` with single Leaf (or splits existing pane for split path), allocates debug port from counter (starting at 19300).
4. **Spawn**: All three browser entry paths (`create_browser_session`, `split_with_browser`, `spawn_restored_browser_pane`) route through a single `spawn_carbonyl_for(terminal, session_id, url)` helper that allocates the debug port, builds env vars, and runs `VteTerminal::spawn_command(["carbonyl", "--remote-debugging-port=<port>", url])`. The `=` form is required: with the space-separated form, Chromium's argument parser routes the URL as a second positional target and exits via `headless_shell.cc`.
5. **Signal wiring**: Connects VTE child-exited → `close_pane` (DES-002), title-changed (page titles), pane_focus, bell, status handlers.
6. **Browser state**: Registers `BrowserPaneState` in `browser_panes` HashMap, starts URL poll via background thread (DES-011).
7. **Sidebar**: Initial display shows globe icon with URL as both title and subtitle.
8. **URL normalization**: `normalize_url(url)` trims whitespace, returns None if empty, prepends `https://` if no `://` scheme present.

### 9. URL + Title Tracking (per pane, DES-011)

1. **Background thread** (spawned at pane creation): Loops every 500ms — HTTP GET to `http://127.0.0.1:{debug_port}/json/list`, parses JSON array, finds first target where `type == "page"`, extracts `url` and `title` fields, sends `PollResult` through mpsc channel. Stops when `AtomicBool` flag is set.
2. **Main thread** (GTK timer, 50ms): Non-blocking `try_recv()` from channel. On result: compares URL and title against current `BrowserPaneState`. If changed and pane is focused: updates sidebar display and marks persistence dirty (DES-003).
3. **Failure handling**: Background thread silently retries on HTTP failure. If channel disconnects (thread exited), GTK timer returns `Break`.

### 10. Browser Pane Destruction

1. **Entry**: Carbonyl process exits → VTE `child-exited` signal → `close_pane`.
2. **Crash detection**: If `created_at.elapsed() < 2s`, treated as crash → `on_browser_error` callback fires with error message.
3. **Cleanup**: Set `AtomicBool` stop flag (terminates background thread), remove GTK poll timer, remove `BrowserPaneState` from HashMap, remove CWD entry.
4. **Session continuity**: If other panes exist, rebuild widget tree and focus surviving pane. If last pane, `destroy_session`.

## Key Decisions

### Decision 1: Vec-based Session Storage with Linear Lookup

**Choice**: Sessions are stored in a `Vec<Session>` with `HashMap<String, SplitView>` for split views. Session lookups by ID use `.iter().find()`.

**Why**: The number of sessions in a terminal multiplexer is small (typically < 50). Linear scan is simple, avoids the overhead of maintaining a HashMap for sessions, and preserves insertion order naturally.

**Alternatives Not Chosen**: `HashMap<String, Session>` would give O(1) lookup but lose insertion ordering and add complexity for ordered iteration. `IndexMap` would give both but adds a dependency for marginal benefit.

**Consequences**: Lookup is O(n) but n is small. Sidebar ordering is the authoritative order for display and persistence, so the session vec order is secondary.

**ADR/DES Candidate**: No -- this is a straightforward data structure choice with negligible impact.

### Decision 2: Dual API Pattern (Instance Methods vs Static Methods with Rc<RefCell>)

**Choice**: Most methods take `&mut self`, but operations that wire GTK signal handlers (split, child-exited, focus, bell, status) take `&Rc<RefCell<Self>>` as a static method parameter.

**Why**: GTK signal handlers are closures that outlive the method call. They need a reference to the manager to call back into it when events fire. Capturing `self` is not possible because the borrow would conflict with other operations. The `Rc<RefCell>` pattern allows the closure to hold a weak or strong reference and borrow at runtime.

**Alternatives Not Chosen**: Message-passing (channel-based) would avoid shared mutable state but adds complexity and latency for UI updates. A global singleton would simplify access but is unidiomatic in Rust.

**Consequences**: Callers must know which API style to use. Signal handlers use `try_borrow` to avoid panics from re-entrant access. The `Rc::downgrade` pattern in focus and bell handlers prevents reference cycles.

**ADR/DES Candidate**: Yes -- DES. This `Rc<RefCell<T>>` with weak references pattern is used throughout the codebase for GTK signal handler wiring and should be documented as a repeatable pattern.

### Decision 3: VTE Title Heuristics for Command Detection

**Choice**: Running command detection for non-Claude sessions relies on parsing VTE window titles. Shell prompts are identified by the `user@host:/path` pattern; anything else is treated as a running command.

**Why**: VTE terminals report the window title set by the shell (via escape sequences). Most shells set the title to `user@host:cwd` at the prompt and to the command name while running. This provides command detection without modifying the user's shell configuration.

**Alternatives Not Chosen**: Shell integration (precmd/preexec hooks) would be more reliable but requires user shell modification. Monitoring `/proc` for child processes would work but is platform-specific and complex.

**Consequences**: The heuristic fails for shells that do not set titles in the `user@host:path` format, or for programs that set custom window titles. The 3-second delay for the "Running" pill mitigates false positives from short-lived commands (cd, ls). Claude sessions bypass this entirely since hook events provide authoritative status.

**ADR/DES Candidate**: Yes -- ADR. This is a significant architectural choice with clear trade-offs that affect user experience.

### Decision 4: Deferred Shell Spawning for Collapsed Groups

**Choice**: When restoring sessions, shells are not spawned for sessions in collapsed groups. Spawning occurs only when the group is expanded or when a specific session is switched to.

**Why**: Users may have many session groups but only work with a few at a time. Spawning all shells at startup would waste resources (memory, process slots) and slow down the restore.

**Alternatives Not Chosen**: Eager spawning of all sessions would be simpler but wasteful. Background spawning on a timer would add complexity.

**Consequences**: First access to a collapsed group has a brief delay while shells spawn. The `needs_spawn` flag on `SplitView` tracks whether spawning has occurred. Pending Claude resume IDs are only consumed for non-collapsed groups (or when a group is explicitly expanded).

**ADR/DES Candidate**: No -- this is a well-understood lazy initialization pattern. The implementation is localized and straightforward.

### Decision 5: Per-Pane CWD Tracking via Shared HashMap

**Choice**: Working directories are tracked in a shared `Rc<RefCell<HashMap<String, String>>>` keyed by pane ID, updated by VTE CWD-changed signals, and read at save time and split time.

**Why**: CWDs need to be accessible from multiple contexts: VTE signal handlers (write), save_state (read), split_active_pane (read for CWD inheritance), and spawn_restored_panes (read). A shared map avoids threading CWD through every call path.

**Alternatives Not Chosen**: Storing CWD on the Session struct would not work because sessions can have multiple panes with different CWDs. Storing CWD on VteTerminal and querying it directly would couple the manager to terminal internals and is unreliable (the URI may not be set yet at spawn time for restored sessions).

**Consequences**: The map must be kept in sync -- entries are added on CWD change, removed on pane close, and bulk-removed on session destroy. The pane ID (not session ID) is the key, which correctly handles multiple panes per session.

**ADR/DES Candidate**: No -- this is a localized implementation detail.

## System Behavior

### Session Lifecycle

- **Given** no sessions exist, **when** a session is created, **then** it receives "Tab 1", a UUID, Idle status, and "default" group. If the GTK stack is realized, the shell spawns immediately; otherwise, spawning is deferred.

### Focus Redirection After Close

- **Given** the active session is in a group with other tabs, **when** it is destroyed, **then** focus moves to the previous sibling in the group. If no previous sibling exists, focus moves to the next sibling. If the group becomes empty, focus moves to the first visible tab in any group.

### Split Pane CWD Inheritance

- **Given** a session with a focused pane at `/home/user/project`, **when** a horizontal split is requested, **then** a new pane appears below, with its shell started in `/home/user/project`.

### Non-Claude Running Detection

- **Given** a non-Claude session at a shell prompt, **when** the user runs `cargo build`, **then** after 3 seconds a "Running" pill appears. **When** the command finishes and the shell prompt returns, **then** if the tab is in the background, a notification with "$ cargo build" is emitted and the pill resets to Idle. If the command finishes before 3 seconds, no pill is shown.

### Git Branch Re-detection After Git Commands

- **Given** a session where the user runs `git checkout feature-branch`, **when** the command finishes (shell prompt returns), **then** after a 2-second debounce, the git branch and PR number are re-detected and the tab updates.

### Bell Debouncing

- **Given** a background session, **when** a terminal bell fires, **then** a notification is created. **When** another bell fires within 2 seconds for the same session, **then** it is discarded.

### Persistence Round-Trip

- **Given** sessions with splits and Claude session IDs in multiple groups, **when** the app closes, **then** session state is saved to JSON. **When** the app restarts, **then** sessions are recreated with their split trees, CWDs, and group membership. Claude session IDs become `pending_resume_id` values, consumed by the hook system to inject `--resume` flags.

### Deferred Spawning on Group Expand

- **Given** a collapsed group with 5 restored sessions, **when** the user expands the group, **then** shells for those 5 sessions are spawned. **When** the user switches to one of those sessions, **then** the terminal is ready with the correct CWD.

### Browser Pane Exits in Split Session

- **Given** a session with both shell and browser panes, **when** Carbonyl exits, **then** the browser pane closes, the shell pane continues, the sidebar updates to show shell metadata (folder icon + CWD + git branch), and the URL poll background thread is stopped.

### Browser Pane Exits in Browser-Only Session

- **Given** a browser-only session (single pane), **when** Carbonyl exits, **then** the last pane closes, the session is destroyed, and focus moves to the next appropriate session.

### Browser Pane Crash

- **Given** a browser pane that crashes within 2 seconds of creation, **when** close_pane runs, **then** an error overlay is shown with crash details (URL, debug port) via `on_browser_error` callback, before the standard pane cleanup.

### Carbonyl Not Found

- **Given** Carbonyl is not in PATH, **when** a browser session or split is attempted, **then** an error overlay appears with installation instructions. No session or pane is created. Availability is cached after first check.

### Restore Browser Pane

- **Given** a saved browser pane with URL, **when** restoring and Carbonyl is available, **then** the browser is re-spawned with the saved URL and URL polling starts. **When** Carbonyl is not available, the pane is skipped (warning to stderr) and other panes in the session restore normally.

### Multiple Browser Panes

- **Given** multiple browser panes across sessions, **then** each has its own CDP debug port, background poll thread, and `BrowserPaneState`. Closing one browser pane does not affect others.

### Pane Focus Change in Mixed Session

- **Given** a session with both shell and browser panes, **when** focus changes between panes, **then** the sidebar tab row updates to reflect the focused pane's type: globe icon + page title + URL for browser panes, folder icon + CWD + git branch for shell panes.

## Notes

- **Uncertainties**: The `move_session_to_position` method accepts a `_position` parameter that is currently ignored -- it delegates to `move_session_to_group` without using the position. This may be an incomplete feature or a simplified API.
- **Assumptions**: The two `on_title_changed` handlers per pane (one in `wire_vte_signals` for tab title/CWD, one in `wire_pane_status` for running detection) both fire independently. This works because GTK4 supports multiple signal connections, but it means title parsing logic is duplicated across two closures with different concerns.
- **Areas needing clarification**: The `Exited` status uses the same CSS class as `Idle` (`status-pill--idle`). It is unclear whether this is intentional (exited sessions should look idle) or an oversight.
