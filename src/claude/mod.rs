pub mod hook_script;

use std::path::PathBuf;

/// Set up the Claude wrapper scripts at runtime.
/// Returns the bin directory path to prepend to PATH.
pub fn setup_scripts(socket_path: &PathBuf) -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

    let bin_dir = PathBuf::from(runtime_dir).join("seemux").join("bin");
    std::fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

    let hook_script_path = bin_dir.join("seemux-hook.sh");
    let claude_wrapper_path = bin_dir.join("claude");

    // Write hook script
    std::fs::write(&hook_script_path, hook_script::hook_script(socket_path))
        .expect("Failed to write hook script");
    set_executable(&hook_script_path);

    // Write claude wrapper
    std::fs::write(&claude_wrapper_path, hook_script::claude_wrapper(&bin_dir, &hook_script_path))
        .expect("Failed to write claude wrapper");
    set_executable(&claude_wrapper_path);

    bin_dir
}

fn set_executable(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("set permissions");
}
