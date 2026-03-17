use std::path::PathBuf;

/// Return the seemux runtime directory: `$XDG_RUNTIME_DIR/seemux/`.
pub fn runtime_dir() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

    PathBuf::from(base).join("seemux")
}

/// Deploy the tmux shim binary to `$XDG_RUNTIME_DIR/seemux/bin/tmux`.
/// The shim binary is expected next to the main seemux binary.
pub fn deploy_tmux_shim() {
    let bin_dir = runtime_dir().join("bin");
    let _ = std::fs::create_dir_all(&bin_dir);

    let target = bin_dir.join("tmux");

    let Ok(exe) = std::env::current_exe() else { return };

    let shim_source = exe.parent()
        .map(|p| p.join("seemux-tmux-shim"))
        .unwrap_or_default();

    if shim_source.exists() {
        let _ = std::fs::remove_file(&target);

        if std::os::unix::fs::symlink(&shim_source, &target).is_err() {
            let _ = std::fs::copy(&shim_source, &target);
        }
    } else {
        eprintln!(
            "agent_teams_shim enabled but shim binary not found at {}",
            shim_source.display(),
        );
    }
}
