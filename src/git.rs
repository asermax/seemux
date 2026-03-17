use std::process::Command;

use gtk4::glib;

pub struct PrInfo {
    pub number: u64,
    pub url: String,
}

/// Detect the git branch for a given directory asynchronously.
/// Runs `git rev-parse` in a background thread and calls `callback` on the main thread.
pub fn detect_branch_async<F: Fn(Option<String>) + 'static>(cwd: &str, callback: F) {
    let cwd = cwd.to_string();

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let branch = Command::new("git")
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
            });

        let _ = tx.send(branch);
    });

    // Poll the result on the main thread
    glib::idle_add_local_once(move || {
        if let Ok(branch) = rx.recv() {
            callback(branch);
        }
    });
}

/// Detect the GitHub PR for the current branch asynchronously.
/// Runs `gh pr view --json number,url` in a background thread and calls `callback` on the main thread.
pub fn detect_pr_async<F: Fn(Option<PrInfo>) + 'static>(cwd: &str, callback: F) {
    let cwd = cwd.to_string();

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let pr_info = Command::new("gh")
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
            });

        let _ = tx.send(pr_info);
    });

    glib::idle_add_local_once(move || {
        if let Ok(pr_info) = rx.recv() {
            callback(pr_info);
        }
    });
}
