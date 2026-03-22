use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use crate::config::Config;
use crate::notifications::hook_server::{HookServer, SocketMessage};
use crate::toplevel_monitor::{ToplevelEvent, ToplevelMonitor};

/// Shared application state across all windows.
pub struct AppState {
    pub config: Rc<RefCell<Config>>,
    /// The socket message receiver — taken by the first window that sets up polling.
    pub hook_rx: RefCell<Option<mpsc::Receiver<SocketMessage>>>,
    /// Toplevel events from the Wayland foreign-toplevel-list protocol — taken
    /// by the quake window to detect external dialogs.
    pub toplevel_rx: RefCell<Option<mpsc::Receiver<ToplevelEvent>>>,
    pub socket_path: PathBuf,
    pub quake: bool,
    _hook_server: HookServer,
}

impl AppState {
    pub fn new(quake: bool) -> Self {
        let config = Rc::new(RefCell::new(Config::load()));

        let hook_server = HookServer::new();
        let socket_path = hook_server.socket_path().clone();
        let hook_rx = hook_server.start();

        if config.borrow().agent_teams_shim {
            crate::runtime::deploy_tmux_shim();
        }

        let toplevel_rx = if quake { ToplevelMonitor::start() } else { None };

        Self {
            config,
            hook_rx: RefCell::new(Some(hook_rx)),
            toplevel_rx: RefCell::new(toplevel_rx),
            socket_path,
            quake,
            _hook_server: hook_server,
        }
    }

    /// Take the socket message receiver (first window claims it).
    pub fn take_hook_rx(&self) -> Option<mpsc::Receiver<SocketMessage>> {
        self.hook_rx.borrow_mut().take()
    }

    /// Take the toplevel event receiver (quake window claims it).
    pub fn take_toplevel_rx(&self) -> Option<mpsc::Receiver<ToplevelEvent>> {
        self.toplevel_rx.borrow_mut().take()
    }
}
