# Tmux Shim — Debugging Procedure

**Companion to**: [tmux-shim.md](tmux-shim.md)
**Status**: Procedure / runbook

## When to Use This

The production shim (`seemux-tmux-shim`) translates a fixed subset of tmux commands into seemux socket calls. When Claude Code's "Agent Teams" backend changes how it drives tmux — new subcommand, new flag, new format string — the production shim falls back to its "unhandled command" branch, returns empty stdout, and Claude silently aborts the teammate spawn.

Reach for this procedure when:

- Teammate spawn fails with vague errors ("Could not determine current tmux pane/window", "Failed to create teammate pane", "tmux not available").
- The team config at `~/.claude/teams/<name>/config.json` records `"backendType": "in-process"` even with `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` set — usually means `process.env.TMUX` wasn't set when `claude` started.
- `seemux-tmux-shim: unhandled command: tmux <subcmd>` shows up in seemux stderr.
- You suspect Claude Code's tmux protocol shifted (recent Claude Code update).
- The shim **hangs** on a teammate spawn (the `send-keys` tmux call never returns). This is not a Claude protocol change — it means the shim's socket request was rejected by seemux without a reply (see "Two protocols" below), so the shim blocks forever reading the response.

## Two Protocols — Don't Conflate Them

There are *two independent* protocols in play, and a spawn failure can come from either:

1. **Claude Code → tmux (argv).** What `claude` passes to the `tmux` shim binary. Changes here = new subcommand / flag / format string → fix `handle_tmux_command`. Diagnose with the debug shim + capture (below).
2. **Shim → seemux (socket).** The JSON the shim writes to `seemux.sock`. This is seemux's *own* internal protocol (`src/notifications/hook_server.rs` + `src/app/hooks/commands.rs`), shared with the hooks plugin. It is **JSON-RPC 2.0**: requests are `{"jsonrpc":"2.0","id":<req>,"method":<cmd>,"params":{…}}`; responses are `{"jsonrpc":"2.0","id":<req>,"result":{…}}` or `{…,"error":{"code":…,"message":…}}`.

The server **drops any line that fails to deserialize as JSON-RPC 2.0 without replying** (`hook_server.rs`: "Failed to parse socket message as JSON-RPC 2.0" → `continue`). So if seemux migrates its socket protocol and the shim's `send_socket_command` is not updated in lockstep, the shim writes a malformed request, gets no reply, and **hangs on `reader.lines().next()`**. Symptom: `send-keys` hangs, `%N` stays `__pending__` in `pane-map.json`, titles pile up in `pending-titles.json`. Fix: re-sync the envelope in `seemux_tmux_shim.rs::send_socket_command` with `hook_server.rs`. (This was the 2026-05-28 breakage — the generic-protocol migration in `2dd5c1f` moved the socket to JSON-RPC 2.0 but left the shim speaking the old `{request_id,command,params}`/`{status,data}` envelope.)

To tell the two apart quickly: run a captured `send-keys` claude-launch command through the **production** shim against the live socket with a `timeout`. If it hangs (exit 124) → it's protocol #2 (socket). If it returns but no session appears → it's protocol #1 or a handler gap.

## What the Debug Shim Does

`seemux-tmux-debug-shim` (built from `src/bin/seemux_tmux_debug_shim.rs`) is a logging passthrough:

1. **Logs every invocation** to `$XDG_RUNTIME_DIR/seemux/tmux-debug.jsonl` — argv, env subset, cwd, pid/ppid, stdout, stderr, exit code.
2. **Synthesizes responses** for read-only discovery probes (`display-message #{pane_id}`, `list-panes`, etc.) so Claude proceeds past the discovery phase and we capture the rest of the protocol.
3. **Delegates everything else** to `/usr/bin/tmux` with the seemux `-S <socket>` stripped (real tmux would otherwise open the JSON-line socket and hang). Stdin inherited so control-mode protocols don't deadlock.

It does **not** create real seemux sessions. Captured commands are *what Claude attempted*; nothing actually runs.

## Deploying It

```bash
# Build both shims
cargo build --bin seemux-tmux-shim --bin seemux-tmux-debug-shim

# Swap the runtime symlink to the debug binary
ln -sf "$(pwd)/target/debug/seemux-tmux-debug-shim" \
       "$XDG_RUNTIME_DIR/seemux/bin/tmux"
```

Caveat: `src/runtime.rs::deploy_tmux_shim` re-creates the production symlink on every seemux startup. The swap is per-seemux-session — if you restart seemux you'll need to swap again.

## Activating Agent Teams in the Captured Shell

In a seemux pane where you want to capture:

```bash
# Confirm the wrapper is the active tmux
which tmux        # → $XDG_RUNTIME_DIR/seemux/bin/tmux

# Set TMUX env — this is the trigger Claude Code uses to register the tmux backend
seemux-agents-on  # alias for: eval "$(tmux seemux-env)"
echo "$TMUX"      # → /run/user/<uid>/seemux/seemux.sock,<pid>,0

# Optional: clean slate
: > "$XDG_RUNTIME_DIR/seemux/tmux-debug.jsonl"
rm -f "$XDG_RUNTIME_DIR/seemux/"{pane-map.json,pending-titles.json,shim-pane-counter}

# Launch a fresh claude in this shell
claude
```

**Important**: TMUX must be set *before* `claude` starts. Claude Code reads `process.env.TMUX` at module load and caches the result — `eval`-ing `seemux-env` after claude is running has no effect.

Inside that claude, trigger a teammate spawn (e.g., "create a team named `probe` and spawn a teammate that runs `echo hi`").

## Reading the Log

Each line is a JSON object:

```json
{"ts":"…","event":"invoke","pid":…,"ppid":…,"cwd":"…","argv":[…],"env":{…},"stdin_is_tty":false}
{"ts":"…","event":"outcome","pid":…,"exit":0,"stdout":"…","stderr":"…","handled":"synth-…"}
```

| Field | Meaning |
|---|---|
| `event` | `invoke` (entry), `outcome` (exit), `spawn_err`/`wait_err` (wrapper failed to run real tmux) |
| `argv` | Full argument vector as Claude passed it — the source of truth for what changed |
| `env.TMUX` | Confirms Claude Code is using the seemux socket path |
| `cwd` | Where the call was made from |
| `handled` | If present, the wrapper synthesized the response. Tags: `synth-display-message`, `synth-list-panes`, `synth-split-window`, `synth-new-window`, `synth-ack-<subcmd>`, `seemux-env`. Absent → delegated to real tmux. |
| `stdout` / `stderr` | What the caller saw |
| `exit` | Real or synthesized exit code |

Quick scan:

```bash
jq -c '{ev:.event, argv, exit, stdout:(.stdout//""|.[0:200]), handled}' \
   "$XDG_RUNTIME_DIR/seemux/tmux-debug.jsonl"
```

To find what's new since the last protocol baseline, compare `argv` shapes against the captured protocol below.

## Captured Protocol (Reference)

This is the teammate-spawn sequence Claude Code drives, in order. Captured 2026-05-11 against Claude Code build dated 2026-05-09 with `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`.

> **Re-capture 2026-05-28 (Claude Code 2.1.154):** the argv sequence is unchanged except for one added probe — `display-message -p '#{window_id}'` right after step 1 (already handled by `cmd_display_message` → `@0`). The `--model` flag now carries `claude-opus-4-8`. The breakage that prompted this re-capture was *not* in this protocol; see "Two protocols" above.

Every call carries `-S /run/user/<uid>/seemux/seemux.sock` (omitted below for readability).

| # | Command | Purpose | Production shim handler |
|---|---|---|---|
| 1 | `display-message -p '#{pane_id}'` | discover orchestrator pane | `cmd_display_message` → `%0` |
| 2 | `list-panes -t @0 -F '#{pane_id}'` | check pane count in current window | `cmd_list_panes` |
| 3 | `split-window -t %0 -h -l 70% -P -F '#{pane_id}'` | create teammate pane | `cmd_split_window` → new `%N` |
| 4 | `select-pane -t %1 -P 'bg=default,fg=colour208'` | cosmetic pane colors | acked silently |
| 5 | `set-option -p -t %1 pane-border-style 'fg=colour208'` | cosmetic | acked silently |
| 6 | `set-option -p -t %1 pane-active-border-style 'fg=colour208'` | cosmetic | acked silently |
| 7 | `select-pane -t %1 -T '<agent-name>'` | set pane title | `cmd_select_pane` stashes title |
| 8 | `set-option -p -t %1 pane-border-format '…'` | cosmetic | acked silently |
| 9 | `list-panes -t @0 -F '#{pane_id}'` | re-check | same as #2 |
| 10 | `set-option -w -t @0 pane-border-status top` | cosmetic | acked silently |
| 11 | `send-keys -t %1 'cd <cwd> && env CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 /opt/claude-code/bin/claude --agent-id X@team --agent-name X --team-name team --agent-color <c> --parent-session-id <uuid> --agent-type <t> --dangerously-skip-permissions --model <m>' Enter` | launch teammate | `cmd_send_keys` → `create_teammate_session` |

The `claude` CLI flags carried in #11: `--agent-id`, `--agent-name`, `--team-name`, `--agent-color`, `--parent-session-id`, `--agent-type`, `--model` (and `--dangerously-skip-permissions`). All but `--dangerously-skip-permissions` are extracted by `extract_flag_from_command` in the production shim and forwarded as `create-session` params.

Teardown (when Claude shuts the teammate down):

- `kill-pane -t %N` → `cmd_kill_pane` removes the pane map entry and emits `destroy-session` on the seemux socket.

## Deriving Shim Diffs from a Trace

When a capture shows the spawn breaking, the diagnostic loop is:

1. Find the first `event:invoke` with no matching synthesized handler (no `handled` tag on the next `outcome`) or with a non-zero `exit`.
2. Compare its `argv` to the table above.
3. If it's a new subcommand: add a `match` arm in `handle_tmux_command`.
4. If it's a new format string for an existing subcommand: extend the format `match` (e.g., in `cmd_display_message` or `cmd_list_panes`).
5. If it's a new flag on `send-keys`'s claude command: add an `extract_flag_from_command` call in `create_teammate_session` and forward it as a `create-session` param. Server-side `cmd_create_session` tolerates extras, so no socket-protocol change needed for capture-only fields.

## Cleanup

```bash
# Restore production symlink (or restart seemux to re-deploy)
ln -sf /usr/bin/seemux-tmux-shim "$XDG_RUNTIME_DIR/seemux/bin/tmux"

# Clear leftover state
rm -f "$XDG_RUNTIME_DIR/seemux/"{tmux-debug.jsonl,shim-pane-counter,pending-titles.json,pane-map.json}
```

Phantom team-config entries the debug shim left behind (teammates Claude *thinks* it spawned but never actually ran) live at `~/.claude/teams/<team-name>/config.json` — delete them via `TeamDelete` from inside claude or `rm -rf ~/.claude/teams/<team-name>` directly.
