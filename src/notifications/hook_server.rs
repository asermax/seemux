use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use super::hook_handler::HookEvent;

pub struct HookServer {
    socket_path: PathBuf,
}

impl HookServer {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/tmp/seemux-{}", unsafe { libc::getuid() }));

        let socket_dir = PathBuf::from(runtime_dir).join("seemux");
        std::fs::create_dir_all(&socket_dir).expect("Failed to create socket directory");

        let socket_path = socket_dir.join("seemux.sock");

        // Clean up stale socket
        let _ = std::fs::remove_file(&socket_path);

        Self { socket_path }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Start listening in a background thread. Returns a receiver for hook events.
    pub fn start(&self) -> mpsc::Receiver<HookEvent> {
        let (tx, rx) = mpsc::channel();
        let path = self.socket_path.clone();

        let _ = thread::Builder::new().name("hook-server".into()).spawn(move || {
            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Failed to bind socket at {}: {e}", path.display());
                    return;
                }
            };

            // Set non-blocking with a timeout so the thread can exit when the app closes
            listener.set_nonblocking(false).ok();

            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let tx = tx.clone();

                thread::spawn(move || {
                    let reader = BufReader::new(&stream);

                    for line in reader.lines() {
                        let Ok(line) = line else { break };

                        if line.trim().is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<HookEvent>(&line) {
                            Ok(event) => {
                                let _ = tx.send(event);
                            }
                            Err(e) => {
                                eprintln!("Failed to parse hook event: {e}");
                            }
                        }
                    }
                });
            }
        });

        rx
    }
}

impl Drop for HookServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
