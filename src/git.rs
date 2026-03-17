use std::process::Command;

use gtk4::glib;

#[derive(Debug, Clone)]
pub struct PrInfo {
    pub number: u64,
    pub url: String,
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

/// Detect the GitHub PR for the current branch asynchronously.
/// Runs `gh pr view --json number,url` in a background thread and calls `callback` on the main thread.
pub fn detect_pr_async<F: Fn(Option<PrInfo>) + 'static>(cwd: &str, callback: F) {
    let cwd = cwd.to_string();

    run_async(
        move || {
            Command::new("gh")
                .args(["pr", "view", "--json", "number,url"])
                .current_dir(&cwd)
                .output()
                .ok()
                .and_then(|o| {
                    if !o.status.success() {
                        return None;
                    }

                    let json: serde_json::Value = serde_json::from_slice(&o.stdout).ok()?;
                    let number = json["number"].as_u64()?;
                    let url = json["url"].as_str()?.to_string();

                    Some(PrInfo { number, url })
                })
        },
        callback,
    );
}
