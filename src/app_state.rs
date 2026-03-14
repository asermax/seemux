use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use crate::claude;
use crate::config::Config;
use crate::notifications::hook_handler::HookEvent;
use crate::notifications::hook_server::HookServer;

/// Shared application state across all windows.
pub struct AppState {
    pub config: Rc<RefCell<Config>>,
    /// The hook receiver — taken by the first window that sets up polling.
    pub hook_rx: RefCell<Option<mpsc::Receiver<HookEvent>>>,
    pub socket_path: PathBuf,
    pub bin_dir: PathBuf,
    _hook_server: HookServer,
}

impl AppState {
    pub fn new() -> Self {
        let config = Rc::new(RefCell::new(Config::load()));

        let hook_server = HookServer::new();
        let socket_path = hook_server.socket_path().clone();
        let hook_rx = hook_server.start();

        let bin_dir = claude::setup_scripts(&socket_path);

        Self {
            config,
            hook_rx: RefCell::new(Some(hook_rx)),
            socket_path,
            bin_dir,
            _hook_server: hook_server,
        }
    }

    /// Take the hook receiver (first window claims it).
    pub fn take_hook_rx(&self) -> Option<mpsc::Receiver<HookEvent>> {
        self.hook_rx.borrow_mut().take()
    }
}
