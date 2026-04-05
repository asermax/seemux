use std::process::Command;

use gtk4::glib;

#[derive(Debug, Clone)]
pub struct PrInfo {
    pub number: u64,
    pub url: String,
}

/// Check if a command string contains a git or gh invocation.
/// Handles `&&` chains, matching any segment that starts with `git ` or `gh `.
pub fn is_git_command(command: &str) -> bool {
    command.split("&&").any(|segment| {
        let trimmed = segment.trim_start();

        trimmed.starts_with("git ") || trimmed.starts_with("gh ")
    })
}

/// Run a closure on a background thread and deliver the result to a callback on the main thread.
/// Uses polling (`try_recv`) to avoid blocking the GTK main loop.
fn run_async<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    callback: impl Fn(T) + 'static,
) {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let _ = tx.send(work());
    });

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        match rx.try_recv() {
            Ok(result) => {
                callback(result);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

/// Detect the git branch for a given directory asynchronously.
/// Runs `git rev-parse` in a background thread and calls `callback` on the main thread.
pub fn detect_branch_async<F: Fn(Option<String>) + 'static>(cwd: &str, callback: F) {
    let cwd = cwd.to_string();

    run_async(
        move || {
            Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&cwd)
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        },
        callback,
    );
}

/// Detect the open GitHub PR for a given branch asynchronously.
/// Runs `gh pr list --head <branch> --state open` in a background thread and calls `callback` on the main thread.
pub fn detect_pr_async<F: Fn(Option<PrInfo>) + 'static>(cwd: &str, branch: &str, callback: F) {
    let cwd = cwd.to_string();
    let branch = branch.to_string();

    run_async(
        move || {
            Command::new("gh")
                .args(["pr", "list", "--head", &branch, "--state", "open", "--json", "number,url", "--limit", "1"])
                .current_dir(&cwd)
                .output()
                .ok()
                .and_then(|o| {
                    if !o.status.success() {
                        return None;
                    }

                    let json: Vec<serde_json::Value> = serde_json::from_slice(&o.stdout).ok()?;
                    let pr = json.first()?;
                    let number = pr["number"].as_u64()?;
                    let url = pr["url"].as_str()?.to_string();

                    Some(PrInfo { number, url })
                })
        },
        callback,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_git_command_detects_simple_commands() {
        assert!(is_git_command("git push"));
        assert!(is_git_command("git add ."));
        assert!(is_git_command("gh pr create"));
        assert!(is_git_command("gh pr view"));
    }

    #[test]
    fn is_git_command_detects_after_double_ampersand() {
        assert!(is_git_command("cd foo && git push"));
        assert!(is_git_command("echo done && gh pr create"));
        assert!(is_git_command("cd foo && git add . && git commit -m 'msg'"));
    }

    #[test]
    fn is_git_command_ignores_non_git_commands() {
        assert!(!is_git_command("cargo build"));
        assert!(!is_git_command("vim"));
        assert!(!is_git_command("cd foo && cargo test"));
        assert!(!is_git_command(""));
    }
}
