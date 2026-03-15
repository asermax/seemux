use std::io::Write;
use std::os::unix::net::UnixStream;

pub enum LaunchMode {
    Normal,
    Quake,
    CommandHandled,
}

/// Parse CLI args. Returns the launch mode for the application.
pub fn handle_args() -> LaunchMode {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        return LaunchMode::Normal;
    }

    match args[1].as_str() {
        "toggle" => {
            send_socket_command("toggle-dropdown");
            LaunchMode::CommandHandled
        }
        "--quake" => LaunchMode::Quake,
        _ => LaunchMode::Normal,
    }
}

fn send_socket_command(command: &str) {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

    let socket_path = format!("{runtime_dir}/seemux/seemux.sock");

    match UnixStream::connect(&socket_path) {
        Ok(mut stream) => {
            let msg = format!("{{\"event\":\"{command}\",\"session_id\":\"\",\"payload\":{{}}}}\n");

            if let Err(e) = stream.write_all(msg.as_bytes()) {
                eprintln!("Failed to send command: {e}");
            }
        }
        Err(e) => {
            eprintln!("Could not connect to seemux socket at {socket_path}: {e}");
            eprintln!("Is seemux running?");
        }
    }
}
