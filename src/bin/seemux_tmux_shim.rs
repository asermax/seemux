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

    let Some(socket_path) = std::env::var_os("SEEMUX_SOCKET") else {
        return exec_real_tmux(&args);
    };

    let socket_path = PathBuf::from(socket_path);
    let runtime_dir = runtime_dir();
    let pane_map_path = runtime_dir.join("pane-map.json");
    let pending_titles_path = runtime_dir.join("pending-titles.json");

    let result = handle_tmux_command(&args, &socket_path, &pane_map_path, &pending_titles_path);

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
    pending_titles_path: &Path,
) -> Result<String, String> {
    if args.is_empty() {
        return Ok(String::new());
    }

    if args.iter().any(|a| a == "-V") {
        return Ok("tmux 3.4".to_string());
    }

    let Some(subcmd_idx) = find_subcmd_index(args) else {
        return Ok(String::new());
    };

    let subcmd = args[subcmd_idx].as_str();
    let sub_args = &args[subcmd_idx + 1..];

    match subcmd {
        "display-message" => cmd_display_message(sub_args, pane_map_path),
        "split-window" | "new-window" => cmd_split_window(sub_args, pane_map_path),
        "send-keys" => cmd_send_keys(sub_args, socket_path, pane_map_path, pending_titles_path),
        "list-panes" => cmd_list_panes(sub_args, pane_map_path),
        "select-pane" => cmd_select_pane(sub_args, pending_titles_path),
        "set-option" | "select-layout" | "resize-pane" | "has-session" => Ok(String::new()),
        "kill-pane" => cmd_kill_pane(sub_args, socket_path, pane_map_path),
        "seemux-env" => cmd_seemux_env(sub_args, socket_path),
        _ => {
            eprintln!("seemux-tmux-shim: unhandled command: tmux {}", args.join(" "));
            Ok(String::new())
        }
    }
}

// --- Pane map (file-locked read-modify-write) ---

/// Execute a closure with exclusive access to a JSON map file.
/// Uses flock to prevent races between concurrent shim invocations.
fn with_locked_map<F, R>(path: &Path, f: F) -> R
where
    F: FnOnce(&mut HashMap<String, String>) -> R,
{
    use std::os::unix::io::AsRawFd;

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap_or_else(|e| panic!("cannot open map at {}: {e}", path.display()));

    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };

    let contents = fs::read_to_string(path).unwrap_or_default();
    let mut map: HashMap<String, String> = serde_json::from_str(&contents).unwrap_or_default();

    let result = f(&mut map);

    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(path, json);
    }

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
    let format = flag_value(args, "-p")
        .or_else(|| args.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str()))
        .unwrap_or("");

    match format {
        "#{pane_id}" => {
            with_locked_map(pane_map_path, |map| {
                if !map.contains_key("%0") {
                    map.insert("%0".to_string(), "__lead__".to_string());
                }
            });

            Ok("%0".to_string())
        }
        "#{window_id}" => Ok("@0".to_string()),
        "#{session_id}" => Ok("$0".to_string()),
        "#{session_name}" => Ok("seemux".to_string()),
        "#{session_name}:#{window_index}" => Ok("seemux:0".to_string()),
        _ => Ok(String::new()),
    }
}

fn cmd_split_window(args: &[String], pane_map_path: &Path) -> Result<String, String> {
    // Allocate a pane ID under lock. The actual session creation happens in send-keys.
    let pane_id = with_locked_map(pane_map_path, |map| {
        let id = next_pane_id(map);
        map.insert(id.clone(), "__pending__".to_string());
        id
    });

    // -P means "print info about new pane" — Claude Code expects the pane ID on stdout
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
    pending_titles_path: &Path,
) -> Result<String, String> {
    let target = flag_value(args, "-t").unwrap_or("").to_string();

    // Collect non-flag keys
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

    let command_parts: Vec<&str> = keys.iter()
        .filter(|k| **k != "Enter" && **k != "C-c" && **k != "C-m")
        .copied()
        .collect();

    let full_command = command_parts.join(" ");

    if full_command.contains("claude") && full_command.contains("--team-name") {
        return create_teammate_session(
            &target,
            &full_command,
            socket_path,
            pane_map_path,
            pending_titles_path,
        );
    }

    // For non-claude commands, send as raw input to the matching session
    let session_id = with_locked_map(pane_map_path, |map| {
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
    pending_titles_path: &Path,
) -> Result<String, String> {
    let team_name = extract_flag_from_command(full_command, "--team-name").unwrap_or_default();
    let agent_name = extract_flag_from_command(full_command, "--agent-name").unwrap_or_default();
    let agent_id = extract_flag_from_command(full_command, "--agent-id");
    let agent_color = extract_flag_from_command(full_command, "--agent-color");
    let agent_type = extract_flag_from_command(full_command, "--agent-type");
    let parent_session_id = extract_flag_from_command(full_command, "--parent-session-id");
    let model = extract_flag_from_command(full_command, "--model");

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

    // Title preference: stashed `select-pane -T` title → --agent-name → "teammate"
    let stashed_title = with_locked_map(pending_titles_path, |map| map.remove(target_pane));

    let title = stashed_title
        .or_else(|| if agent_name.is_empty() { None } else { Some(agent_name) })
        .unwrap_or_else(|| "teammate".to_string());

    let argv = vec!["sh".to_string(), "-c".to_string(), full_command.to_string()];

    let mut params = serde_json::json!({
        "title": title,
        "group_id": group_id,
        "argv": argv,
    });

    if let Some(cwd) = &cwd {
        params["cwd"] = serde_json::Value::String(cwd.clone());
    }

    // Forward the rest of the agent metadata. The server reads params by key
    // and tolerates extras, so unknown keys are safe to include and will be
    // available once the seemux UI grows wiring for them.
    if let Some(id) = agent_id { params["agent_id"] = serde_json::Value::String(id); }
    if let Some(c) = agent_color { params["agent_color"] = serde_json::Value::String(c); }
    if let Some(t) = agent_type { params["agent_type"] = serde_json::Value::String(t); }
    if let Some(p) = parent_session_id { params["parent_session_id"] = serde_json::Value::String(p); }
    if let Some(m) = model { params["model"] = serde_json::Value::String(m); }

    let session_response = send_socket_command(socket_path, "create-session", params)?;

    let session_id = session_response.get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Replace __pending__ with the real session ID in the pane map
    with_locked_map(pane_map_path, |map| {
        map.insert(target_pane.to_string(), session_id);
    });

    Ok(String::new())
}

fn cmd_list_panes(args: &[String], pane_map_path: &Path) -> Result<String, String> {
    let format = flag_value(args, "-F").unwrap_or("#{pane_id}");

    let result = with_locked_map(pane_map_path, |map| {
        let mut pane_ids: Vec<String> = map.keys().cloned().collect();
        pane_ids.sort_by(|a, b| {
            let a_num = a.strip_prefix('%').and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            let b_num = b.strip_prefix('%').and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            a_num.cmp(&b_num)
        });

        match format {
            "#{pane_id}" => pane_ids.join("\n"),
            "#{pane_id} #{pane_active}" => pane_ids.iter()
                .enumerate()
                .map(|(i, id)| format!("{id} {}", if i == 0 { 1 } else { 0 }))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => pane_ids.join("\n"),
        }
    });

    Ok(result)
}

/// Stash `-T <title>` for the target pane so it becomes the session title
/// when send-keys later spawns the teammate. All other select-pane forms
/// (`-P bg=...,fg=...`, etc.) silently ack — seemux owns its own UI styling.
fn cmd_select_pane(args: &[String], pending_titles_path: &Path) -> Result<String, String> {
    let target = flag_value(args, "-t").unwrap_or("");
    let title = flag_value(args, "-T");

    if let (Some(title), false) = (title, target.is_empty()) {
        with_locked_map(pending_titles_path, |map| {
            map.insert(target.to_string(), title.to_string());
        });
    }

    Ok(String::new())
}

fn cmd_kill_pane(
    args: &[String],
    socket_path: &Path,
    pane_map_path: &Path,
) -> Result<String, String> {
    let target = flag_value(args, "-t").unwrap_or("").to_string();

    let session_id = with_locked_map(pane_map_path, |map| {
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

/// Find the index of the first positional arg, skipping the value of any
/// value-taking flag (`-S <path>`, `-L <name>`, `-f <file>`, `-c <cmd>`).
/// This is critical: Claude Code now passes `-S <socket-path>` to every call,
/// and the value is not flag-prefixed.
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

// --- Environment toggle ---

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

fn runtime_dir() -> PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

    PathBuf::from(dir).join("seemux")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn s(v: &str) -> String { v.to_string() }

    #[test]
    fn seemux_env_on_outputs_export() {
        let path = Path::new("/tmp/test.sock");
        let result = cmd_seemux_env(&[], path).unwrap();
        assert!(result.starts_with("export TMUX='/tmp/test.sock,"));
        assert!(result.ends_with(",0';"));
    }

    #[test]
    fn seemux_env_explicit_on_outputs_export() {
        let path = Path::new("/tmp/test.sock");
        let result = cmd_seemux_env(&[s("on")], path).unwrap();
        assert!(result.starts_with("export TMUX='/tmp/test.sock,"));
        assert!(result.ends_with(",0';"));
    }

    #[test]
    fn seemux_env_off_outputs_unset() {
        let path = Path::new("/tmp/test.sock");
        assert_eq!(cmd_seemux_env(&[s("off")], path).unwrap(), "unset TMUX;");
    }

    #[test]
    fn seemux_env_unknown_mode_errors() {
        let path = Path::new("/tmp/test.sock");
        assert!(cmd_seemux_env(&[s("bogus")], path).is_err());
    }

    #[test]
    fn find_subcmd_skips_dash_s_value() {
        let args = vec![s("-S"), s("/run/seemux.sock"), s("display-message"), s("-p"), s("#{pane_id}")];
        let idx = find_subcmd_index(&args).unwrap();
        assert_eq!(args[idx], "display-message");
    }

    #[test]
    fn find_subcmd_skips_dash_l_value() {
        let args = vec![s("-L"), s("myname"), s("list-panes")];
        let idx = find_subcmd_index(&args).unwrap();
        assert_eq!(args[idx], "list-panes");
    }

    #[test]
    fn find_subcmd_returns_none_when_all_flags() {
        let args = vec![s("-V")];
        assert_eq!(find_subcmd_index(&args), None);
    }

    #[test]
    fn display_message_window_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pane-map.json");
        let args = vec![s("-p"), s("#{window_id}")];
        let result = cmd_display_message(&args, &path).unwrap();
        assert_eq!(result, "@0");
    }

    #[test]
    fn display_message_session_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pane-map.json");
        let args = vec![s("-p"), s("#{session_id}")];
        let result = cmd_display_message(&args, &path).unwrap();
        assert_eq!(result, "$0");
    }

    #[test]
    fn select_pane_stashes_title() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pending-titles.json");
        let args = vec![s("-t"), s("%1"), s("-T"), s("agent-foo")];
        cmd_select_pane(&args, &path).unwrap();

        let stashed = with_locked_map(&path, |map| map.get("%1").cloned());
        assert_eq!(stashed, Some("agent-foo".to_string()));
    }

    #[test]
    fn select_pane_without_title_acks_silently() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pending-titles.json");
        let args = vec![s("-t"), s("%1"), s("-P"), s("bg=default,fg=colour208")];
        let result = cmd_select_pane(&args, &path).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn extract_flag_handles_new_agent_flags() {
        let cmd = "cd /x && env A=1 claude --team-name t --agent-name a --agent-color orange --parent-session-id abc-123 --model claude-opus-4-7";
        assert_eq!(extract_flag_from_command(cmd, "--agent-color"), Some(s("orange")));
        assert_eq!(extract_flag_from_command(cmd, "--parent-session-id"), Some(s("abc-123")));
        assert_eq!(extract_flag_from_command(cmd, "--model"), Some(s("claude-opus-4-7")));
    }

    #[test]
    fn list_panes_pane_id_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pane-map.json");
        with_locked_map(&path, |map| {
            map.insert(s("%0"), s("__lead__"));
            map.insert(s("%1"), s("session-uuid"));
        });

        let args = vec![s("-t"), s("@0"), s("-F"), s("#{pane_id}")];
        let result = cmd_list_panes(&args, &path).unwrap();
        assert_eq!(result, "%0\n%1");
    }

    #[test]
    fn list_panes_with_active_flag() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pane-map.json");
        with_locked_map(&path, |map| {
            map.insert(s("%0"), s("__lead__"));
            map.insert(s("%1"), s("session-uuid"));
        });

        let args = vec![s("-F"), s("#{pane_id} #{pane_active}")];
        let result = cmd_list_panes(&args, &path).unwrap();
        assert_eq!(result, "%0 1\n%1 0");
    }
}
