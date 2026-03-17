//! Seemux tmux shim — intercepts tmux commands from Claude Code Agent Teams
//! and translates them into seemux socket commands.
//!
//! When `$SEEMUX_SOCKET` is set, this binary handles tmux commands by creating
//! seemux sessions/groups via the socket protocol. When not set, it falls through
//! to the real tmux binary.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Fall through to real tmux if not inside seemux
    let Some(socket_path) = std::env::var_os("SEEMUX_SOCKET") else {
        return exec_real_tmux(&args);
    };

    let socket_path = PathBuf::from(socket_path);
    let runtime_dir = runtime_dir();
    let pane_map_path = runtime_dir.join("pane-map.json");

    let result = handle_tmux_command(&args, &socket_path, &pane_map_path);

    match result {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("seemux-tmux-shim: {e}");
            ExitCode::FAILURE
        }
    }
}

fn handle_tmux_command(
    args: &[String],
    socket_path: &Path,
    pane_map_path: &Path,
) -> Result<String, String> {
    if args.is_empty() {
        return Ok(String::new());
    }

    // Handle `-V` anywhere in args
    if args.iter().any(|a| a == "-V") {
        return Ok("tmux 3.4".to_string());
    }

    // Find the subcommand (first non-flag argument)
    let (subcmd_idx, subcmd) = args.iter().enumerate()
        .find(|(_, a)| !a.starts_with('-'))
        .map(|(i, a)| (i, a.as_str()))
        .unwrap_or((0, ""));

    let sub_args = &args[subcmd_idx + 1..];

    match subcmd {
        "display-message" => cmd_display_message(sub_args, pane_map_path),
        "split-window" | "new-window" => cmd_split_window(sub_args, socket_path, pane_map_path),
        "send-keys" => cmd_send_keys(sub_args, socket_path, pane_map_path),
        "list-panes" => cmd_list_panes(sub_args, pane_map_path),
        "select-pane" => Ok(String::new()),
        "set-option" => Ok(String::new()),
        "select-layout" => Ok(String::new()),
        "resize-pane" => Ok(String::new()),
        "kill-pane" => cmd_kill_pane(sub_args, socket_path, pane_map_path),
        "has-session" => Ok(String::new()),
        _ => {
            eprintln!("seemux-tmux-shim: unhandled command: tmux {}", args.join(" "));
            Ok(String::new())
        }
    }
}

// --- Pane map (file-locked read-modify-write) ---

/// Execute a closure with exclusive access to the pane map.
/// Uses flock to prevent races between concurrent shim invocations.
fn with_pane_map<F, R>(path: &Path, f: F) -> R
where
    F: FnOnce(&mut HashMap<String, String>) -> R,
{
    use std::os::unix::io::AsRawFd;

    // Open (or create) the file and take an exclusive lock
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap_or_else(|e| panic!("cannot open pane map at {}: {e}", path.display()));

    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };

    let contents = fs::read_to_string(path).unwrap_or_default();
    let mut map: HashMap<String, String> = serde_json::from_str(&contents).unwrap_or_default();

    let result = f(&mut map);

    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(path, json);
    }

    // Lock released on file drop
    result
}

fn next_pane_id(map: &HashMap<String, String>) -> String {
    let max = map.keys()
        .filter_map(|k| k.strip_prefix('%').and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);

    format!("%{}", max + 1)
}

// --- Command handlers ---

fn cmd_display_message(args: &[String], pane_map_path: &Path) -> Result<String, String> {
    let format = find_positional_arg(args);

    match format {
        "#{pane_id}" => {
            with_pane_map(pane_map_path, |map| {
                if !map.contains_key("%0") {
                    map.insert("%0".to_string(), "__lead__".to_string());
                }
            });

            Ok("%0".to_string())
        }
        "#{session_name}:#{window_index}" => Ok("seemux:0".to_string()),
        _ => Ok(String::new()),
    }
}

fn cmd_split_window(
    args: &[String],
    _socket_path: &Path,
    pane_map_path: &Path,
) -> Result<String, String> {
    // Allocate a pane ID under lock. The actual session creation happens in send-keys.
    let pane_id = with_pane_map(pane_map_path, |map| {
        let id = next_pane_id(map);
        map.insert(id.clone(), "__pending__".to_string());
        id
    });

    // Check if -P flag is present (print pane info) — Claude Code expects the pane ID on stdout
    if args.iter().any(|a| a == "-P") {
        Ok(pane_id)
    } else {
        Ok(String::new())
    }
}

fn cmd_send_keys(
    args: &[String],
    socket_path: &Path,
    pane_map_path: &Path,
) -> Result<String, String> {
    // Parse: send-keys -t %N key1 key2 ... Enter
    let target = find_flag_value(args, "-t").unwrap_or_default();

    // Collect all keys (args after -t TARGET that aren't flags)
    let keys: Vec<&str> = {
        let mut result = Vec::new();
        let mut skip_next = false;

        for (i, arg) in args.iter().enumerate() {
            if skip_next {
                skip_next = false;
                continue;
            }

            if arg == "-t" {
                skip_next = true;
                continue;
            }

            if i == 0 && arg.starts_with('-') {
                continue;
            }

            result.push(arg.as_str());
        }

        result
    };

    // Filter out the trailing "Enter" key name
    let command_parts: Vec<&str> = keys.iter()
        .filter(|k| **k != "Enter" && **k != "C-c" && **k != "C-m")
        .copied()
        .collect();

    let full_command = command_parts.join(" ");

    // Check if this looks like a Claude teammate launch command
    if full_command.contains("claude") && full_command.contains("--team-name") {
        return create_teammate_session(&target, &full_command, socket_path, pane_map_path);
    }

    // For non-claude commands, send as raw input to the terminal
    let session_id = with_pane_map(pane_map_path, |map| {
        map.get(&target)
            .filter(|id| *id != "__pending__" && *id != "__lead__")
            .cloned()
    });

    if let Some(session_id) = session_id {
        let text = if keys.last() == Some(&"Enter") {
            format!("{full_command}\n")
        } else {
            full_command
        };

        send_socket_command(socket_path, "send-input", serde_json::json!({
            "session_id": session_id,
            "text": text,
        }))?;
    }

    Ok(String::new())
}

fn create_teammate_session(
    target_pane: &str,
    full_command: &str,
    socket_path: &Path,
    pane_map_path: &Path,
) -> Result<String, String> {
    // Extract team-name and agent-name from the command
    let team_name = extract_flag_from_command(full_command, "--team-name").unwrap_or_default();
    let agent_name = extract_flag_from_command(full_command, "--agent-name").unwrap_or_default();

    // Extract cwd from `cd /path &&` prefix
    let cwd = if full_command.starts_with("cd ") {
        full_command.strip_prefix("cd ")
            .and_then(|s| s.split(" && ").next())
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    // Create or find the team group
    let group_name = format!("Team: {team_name}");
    let mut group_params = serde_json::json!({ "name": group_name });

    if let Ok(sid) = std::env::var("SEEMUX_SESSION_ID") {
        group_params["source_session_id"] = serde_json::Value::String(sid);
    }

    let group_response = send_socket_command(socket_path, "create-group", group_params)?;

    let group_id = group_response.get("group_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    // Build the session title
    let title = if agent_name.is_empty() { "teammate".to_string() } else { agent_name };

    // Run the full command via sh -c so cd, env, and && all work
    let argv = vec!["sh".to_string(), "-c".to_string(), full_command.to_string()];

    let mut params = serde_json::json!({
        "title": title,
        "group_id": group_id,
        "argv": argv,
    });

    if let Some(cwd) = &cwd {
        params["cwd"] = serde_json::Value::String(cwd.clone());
    }

    let session_response = send_socket_command(socket_path, "create-session", params)?;

    let session_id = session_response.get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Update the pane map: replace __pending__ with the real session ID
    with_pane_map(pane_map_path, |map| {
        map.insert(target_pane.to_string(), session_id);
    });

    Ok(String::new())
}

fn cmd_list_panes(args: &[String], pane_map_path: &Path) -> Result<String, String> {
    let _target = find_flag_value(args, "-t");

    let result = with_pane_map(pane_map_path, |map| {
        let mut pane_ids: Vec<String> = map.keys().cloned().collect();
        pane_ids.sort_by(|a, b| {
            let a_num = a.strip_prefix('%').and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            let b_num = b.strip_prefix('%').and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            a_num.cmp(&b_num)
        });
        pane_ids.join("\n")
    });

    Ok(result)
}

fn cmd_kill_pane(
    args: &[String],
    socket_path: &Path,
    pane_map_path: &Path,
) -> Result<String, String> {
    let target = find_flag_value(args, "-t").unwrap_or_default();

    let session_id = with_pane_map(pane_map_path, |map| {
        map.remove(&target)
            .filter(|id| id != "__pending__" && id != "__lead__")
    });

    if let Some(session_id) = session_id {
        send_socket_command(socket_path, "destroy-session", serde_json::json!({
            "session_id": session_id,
        }))?;
    }

    Ok(String::new())
}

// --- Socket communication ---

fn send_socket_command(
    socket_path: &Path,
    command: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request_id = format!("shim-{}", std::process::id());

    let request = serde_json::json!({
        "request_id": request_id,
        "command": command,
        "params": params,
    });

    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| format!("cannot connect to seemux socket: {e}"))?;

    let msg = format!("{}\n", serde_json::to_string(&request).unwrap());
    stream.write_all(msg.as_bytes())
        .map_err(|e| format!("failed to send command: {e}"))?;
    stream.flush()
        .map_err(|e| format!("failed to flush: {e}"))?;

    // Read response
    let reader = BufReader::new(&stream);
    let line = reader.lines().next()
        .ok_or("no response from seemux")?
        .map_err(|e| format!("failed to read response: {e}"))?;

    let response: serde_json::Value = serde_json::from_str(&line)
        .map_err(|e| format!("failed to parse response: {e}"))?;

    if response.get("status").and_then(|v| v.as_str()) == Some("error") {
        let error = response.get("data")
            .and_then(|d| d.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");

        return Err(format!("command {command} failed: {error}"));
    }

    Ok(response.get("data").cloned().unwrap_or(serde_json::Value::Null))
}

// --- Argument parsing helpers ---

fn find_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}

/// Find the first positional argument (not a flag and not a flag value).
fn find_positional_arg(args: &[String]) -> &str {
    let mut skip_next = false;

    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }

        // Flags that take a value
        if arg == "-t" || arg == "-F" || arg == "-p" {
            if arg == "-p" {
                // -p is a boolean flag for display-message (print to stdout)
                continue;
            }
            skip_next = true;
            continue;
        }

        if arg.starts_with('-') {
            continue;
        }

        return arg;
    }

    ""
}

fn extract_flag_from_command(command: &str, flag: &str) -> Option<String> {
    let parts: Vec<&str> = command.split_whitespace().collect();

    parts.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].replace('\\', ""))
}

// --- Fallthrough to real tmux ---

fn exec_real_tmux(args: &[String]) -> ExitCode {
    let status = Command::new("/usr/bin/tmux")
        .args(args)
        .status();

    match status {
        Ok(s) => {
            if s.success() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(e) => {
            eprintln!("Failed to exec real tmux: {e}");
            ExitCode::FAILURE
        }
    }
}

fn runtime_dir() -> PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

    PathBuf::from(dir).join("seemux")
}
