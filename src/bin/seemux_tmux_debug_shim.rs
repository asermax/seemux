//! Seemux tmux shim — DEBUG / CAPTURE binary.
//!
//! Logs every invocation (argv, env, cwd, stdin-tty flag) and delegates to the
//! real tmux binary, capturing stdout/stderr/exit code into a JSONL log. Use
//! this when you need to see how Claude Code (or any other client) drives tmux
//! through the seemux shim path — typically when teammate spawn fails silently
//! or when the production shim hits an unhandled subcommand.
//!
//! Deploy by symlinking `$XDG_RUNTIME_DIR/seemux/bin/tmux` at this binary
//! instead of the production `seemux-tmux-shim`. See
//! `docs/feature-designs/agent-teams/tmux-shim-debugging.md`.

use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let pid = std::process::id();
    let ppid = unsafe { libc::getppid() };
    let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
    let stdin_is_tty = std::io::stdin().is_terminal();

    log_entry("invoke", serde_json::json!({
        "pid": pid,
        "ppid": ppid,
        "cwd": cwd,
        "argv": args,
        "env": collect_interesting_env(),
        "stdin_is_tty": stdin_is_tty,
    }));

    // Synthesize responses for read-only discovery probes. Without these, Claude
    // aborts the spawn before issuing the interesting commands (new-window,
    // split-window, send-keys) and we never see the rest of the protocol.
    if let Some(synth) = synthesize_discovery(&args) {
        log_entry("outcome", serde_json::json!({
            "pid": pid, "exit": 0, "stdout": synth.0, "stderr": "",
            "handled": synth.1,
        }));
        if !synth.0.is_empty() { println!("{}", synth.0); }
        return ExitCode::SUCCESS;
    }

    // Handle seemux-env locally — it's a seemux-specific subcommand, not real tmux.
    // Without this, `seemux-agents-on` would fail because real tmux doesn't know it.
    if args.first().map(|s| s.as_str()) == Some("seemux-env") {
        let socket = std::env::var("SEEMUX_SOCKET").unwrap_or_default();
        let socket_path = PathBuf::from(socket);
        let result = cmd_seemux_env(&args[1..], &socket_path);

        match result {
            Ok(out) => {
                log_entry("outcome", serde_json::json!({
                    "pid": pid, "exit": 0, "stdout": out, "stderr": "", "handled": "seemux-env",
                }));
                if !out.is_empty() { println!("{out}"); }
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                log_entry("outcome", serde_json::json!({
                    "pid": pid, "exit": 1, "stdout": "", "stderr": e, "handled": "seemux-env",
                }));
                eprintln!("seemux-tmux-debug-shim: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    // Strip the seemux TMUX env and any `-S/-L` args pointing at the seemux socket,
    // so the inner real-tmux call doesn't open the JSON-line socket and hang.
    let filtered_args = strip_seemux_socket_flags(&args);

    let mut cmd = Command::new("/usr/bin/tmux");
    cmd.args(&filtered_args);
    cmd.env_remove("TMUX");
    cmd.env_remove("TMUX_PANE");
    // Inherit stdin so we don't block reading it. Capture stdout/stderr.
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log_entry("spawn_err", serde_json::json!({ "pid": pid, "err": e.to_string() }));
            eprintln!("seemux-tmux-debug-shim: failed to spawn real tmux: {e}");
            return ExitCode::FAILURE;
        }
    };

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            log_entry("wait_err", serde_json::json!({ "pid": pid, "err": e.to_string() }));
            eprintln!("seemux-tmux-debug-shim: failed to wait for real tmux: {e}");
            return ExitCode::FAILURE;
        }
    };

    let exit_code = output.status.code().unwrap_or(-1);

    log_entry("outcome", serde_json::json!({
        "pid": pid,
        "exit": exit_code,
        "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
        "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
    }));

    let _ = std::io::stdout().write_all(&output.stdout);
    let _ = std::io::stderr().write_all(&output.stderr);

    match u8::try_from(exit_code) {
        Ok(c) => ExitCode::from(c),
        Err(_) => ExitCode::FAILURE,
    }
}

// --- JSONL invocation logging ---

fn log_entry(event: &str, mut payload: serde_json::Value) {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("ts".to_string(), serde_json::Value::String(now_ts()));
        obj.insert("event".to_string(), serde_json::Value::String(event.to_string()));
    }

    let path = runtime_dir().join("tmux-debug.jsonl");

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{payload}");
    }
}

fn collect_interesting_env() -> serde_json::Value {
    let keys = [
        "TMUX", "TMUX_PANE", "TMUX_TMPDIR",
        "SEEMUX_SOCKET", "SEEMUX_SESSION_ID",
        "CLAUDE_CODE_SSE_PORT", "CLAUDECODE",
        "CLAUDE_AGENT_NAME", "CLAUDE_TEAM_NAME",
        "PATH", "PWD", "TERM",
    ];

    let mut obj = serde_json::Map::new();

    for k in keys {
        if let Ok(v) = std::env::var(k) {
            obj.insert(k.to_string(), serde_json::Value::String(v));
        }
    }

    serde_json::Value::Object(obj)
}

// --- Discovery response synthesis ---

/// Returns (stdout, handler-tag) when we can synthesize a response.
fn synthesize_discovery(args: &[String]) -> Option<(String, String)> {
    let sub_i = find_subcmd_index(args)?;
    let subcmd = args.get(sub_i)?.as_str();
    let after_sub = &args[sub_i + 1..];

    match subcmd {
        "display-message" => {
            let format = flag_value(after_sub, "-p")
                .or_else(|| after_sub.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str()))?;

            let response = match format {
                "#{pane_id}" => "%0",
                "#{window_id}" => "@0",
                "#{session_id}" => "$0",
                "#{session_name}" => "seemux",
                "#{session_name}:#{window_index}" => "seemux:0",
                "#{client_control_mode}" => "0",
                _ => return Some((String::new(), format!("synth-display-message-unknown-format:{format}"))),
            };

            Some((response.to_string(), "synth-display-message".to_string()))
        }
        "list-panes" => {
            let format = flag_value(after_sub, "-F").unwrap_or("#{pane_id}");
            let response = match format {
                "#{pane_id}" => "%0",
                "#{pane_id} #{pane_active}" => "%0 1",
                _ => return Some((String::new(), format!("synth-list-panes-unknown-format:{format}"))),
            };

            Some((response.to_string(), "synth-list-panes".to_string()))
        }
        "list-windows" => {
            let format = flag_value(after_sub, "-F").unwrap_or("#{window_id}");
            let response = match format {
                "#{window_id}" => "@0",
                _ => return Some((String::new(), format!("synth-list-windows-unknown-format:{format}"))),
            };

            Some((response.to_string(), "synth-list-windows".to_string()))
        }
        "split-window" | "new-window" => {
            let pane_id = allocate_pane_id();
            // -P means "print info about new pane". Without -P, tmux returns no stdout.
            let print = after_sub.iter().any(|a| a == "-P");

            if print {
                Some((pane_id, format!("synth-{subcmd}")))
            } else {
                Some((String::new(), format!("synth-{subcmd}")))
            }
        }
        "send-keys" | "select-pane" | "select-layout" | "set-option"
            | "resize-pane" | "kill-pane" | "has-session" => {
            // Accept silently — these are state-mutating ops we can't faithfully
            // execute against a real tmux server, but Claude expects exit 0.
            Some((String::new(), format!("synth-ack-{subcmd}")))
        }
        _ => None,
    }
}

fn allocate_pane_id() -> String {
    use std::os::unix::io::AsRawFd;

    let path = runtime_dir().join("shim-pane-counter");

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path);

    let Ok(file) = file else { return "%1".to_string() };

    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };

    let contents = fs::read_to_string(&path).unwrap_or_default();
    let current: u32 = contents.trim().parse().unwrap_or(0);
    let next = current + 1;
    let _ = fs::write(&path, next.to_string());

    format!("%{next}")
}

// --- Argument helpers ---

fn find_subcmd_index(args: &[String]) -> Option<usize> {
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];

        if (a == "-S" || a == "-L" || a == "-f" || a == "-c") && i + 1 < args.len() {
            i += 2;
            continue;
        }

        if !a.starts_with('-') {
            return Some(i);
        }

        i += 1;
    }

    None
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .zip(args.iter().skip(1))
        .find(|(a, _)| *a == flag)
        .map(|(_, v)| v.as_str())
}

/// Drop any `-S <path>` / `-L <name>` arg whose value points at the seemux socket.
/// These would otherwise be passed through to real tmux, which would open the
/// seemux unix socket and hang trying to speak the tmux binary protocol over it.
fn strip_seemux_socket_flags(args: &[String]) -> Vec<String> {
    let seemux_socket = std::env::var("SEEMUX_SOCKET").unwrap_or_default();
    let mut out = Vec::with_capacity(args.len());
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];

        if (a == "-S" || a == "-L") && i + 1 < args.len() {
            let v = &args[i + 1];
            let matches_seemux = !seemux_socket.is_empty()
                && (v == &seemux_socket || Path::new(v) == Path::new(&seemux_socket));

            if matches_seemux {
                i += 2;
                continue;
            }
        }

        out.push(a.clone());
        i += 1;
    }

    out
}

// --- seemux-env subcommand (also implemented by the production shim) ---

fn cmd_seemux_env(args: &[String], socket_path: &Path) -> Result<String, String> {
    let mode = args.first().map(|s| s.as_str()).unwrap_or("on");

    match mode {
        "on" => Ok(format!(
            "export TMUX='{},{},0';",
            socket_path.display(),
            std::process::id(),
        )),
        "off" => Ok("unset TMUX;".to_string()),
        other => Err(format!("unknown seemux-env mode: {other} (expected 'on' or 'off')")),
    }
}

// --- Common helpers ---

fn runtime_dir() -> PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

    PathBuf::from(dir).join("seemux")
}

fn now_ts() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{}.{:09}", d.as_secs(), d.subsec_nanos())
}
