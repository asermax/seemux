use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use super::hook_handler::HookEvent;

/// A command request sent over the socket (distinguished by having a `request_id`).
pub struct CommandRequest {
    pub request_id: String,
    pub command: String,
    pub params: serde_json::Value,
    pub response_tx: mpsc::SyncSender<CommandResponse>,
}

/// Response sent back to the socket client after processing a command.
#[derive(serde::Serialize)]
pub struct CommandResponse {
    pub request_id: String,
    pub status: String,
    pub data: serde_json::Value,
}

/// Messages coming in over the socket — either hook events or commands.
pub enum SocketMessage {
    Hook(HookEvent),
    Command(CommandRequest),
}

pub struct HookServer {
    socket_path: PathBuf,
}

impl HookServer {
    pub fn new() -> Self {
        let socket_dir = crate::runtime::runtime_dir();
        std::fs::create_dir_all(&socket_dir).expect("Failed to create socket directory");

        let socket_path = socket_dir.join("seemux.sock");

        // Clean up stale socket
        let _ = std::fs::remove_file(&socket_path);

        Self { socket_path }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Start listening in a background thread. Returns a receiver for socket messages.
    pub fn start(&self) -> mpsc::Receiver<SocketMessage> {
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

                        // Parse once as a generic Value, then route by shape
                        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&line) else {
                            eprintln!("Failed to parse socket message as JSON");
                            continue;
                        };

                        let is_command = value.get("request_id").is_some_and(|v| v.is_string())
                            && value.get("command").is_some_and(|v| v.is_string());

                        if is_command {
                            let request_id = value["request_id"].as_str().unwrap().to_string();
                            let command = value["command"].as_str().unwrap().to_string();
                            let params = value.get_mut("params")
                                .map(|v| v.take())
                                .unwrap_or(serde_json::Value::Null);

                            let (resp_tx, resp_rx) = mpsc::sync_channel(1);

                            let request = CommandRequest {
                                request_id,
                                command,
                                params,
                                response_tx: resp_tx,
                            };

                            if tx.send(SocketMessage::Command(request)).is_err() {
                                break;
                            }

                            // Block waiting for the main thread to process and respond
                            if let Ok(response) = resp_rx.recv()
                                && let Ok(json) = serde_json::to_string(&response)
                                && let Ok(mut writer) = stream.try_clone()
                            {
                                let _ = writeln!(writer, "{json}");
                                let _ = writer.flush();
                            }
                        } else {
                            match serde_json::from_value::<HookEvent>(value) {
                                Ok(event) => {
                                    let _ = tx.send(SocketMessage::Hook(event));
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse hook event: {e}");
                                }
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
