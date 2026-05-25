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

#[derive(serde::Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
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

                        let msg = match serde_json::from_str::<JsonRpcMessage>(&line) {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!("Failed to parse socket message as JSON-RPC 2.0: {e}");
                                continue;
                            }
                        };

                        if msg.jsonrpc != "2.0" {
                            eprintln!("Invalid jsonrpc version: {}", msg.jsonrpc);
                            continue;
                        }

                        if let Some(id_val) = &msg.id {
                            let request_id = match id_val {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                other => other.to_string(),
                            };

                            let (resp_tx, resp_rx) = mpsc::sync_channel(1);

                            let request = CommandRequest {
                                request_id,
                                command: msg.method,
                                params: msg.params,
                                response_tx: resp_tx,
                            };

                            if tx.send(SocketMessage::Command(request)).is_err() {
                                break;
                            }

                            // Block waiting for the main thread to process and respond
                            if let Ok(response) = resp_rx.recv() {
                                let rpc_resp = if response.status == "error" {
                                    serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": response.request_id,
                                        "error": {
                                            "code": -32603,
                                            "message": response.data.get("error").and_then(|e| e.as_str()).unwrap_or("Internal error")
                                        }
                                    })
                                } else {
                                    serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": response.request_id,
                                        "result": response.data
                                    })
                                };

                                if let Ok(json) = serde_json::to_string(&rpc_resp)
                                    && let Ok(mut writer) = stream.try_clone()
                                {
                                    let _ = writeln!(writer, "{json}");
                                    let _ = writer.flush();
                                }
                            }
                        } else {
                            let session_id = msg.params.get("session_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            let event = HookEvent {
                                event: msg.method,
                                session_id,
                                payload: msg.params,
                            };

                            let _ = tx.send(SocketMessage::Hook(event));
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
