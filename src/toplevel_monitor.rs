//! Monitors Wayland toplevels to detect external dialogs.
//!
//! Tries `ext-foreign-toplevel-list-v1` first (wlroots, Hyprland, Sway), then
//! falls back to `org_kde_plasma_window_management` (KWin/KDE Plasma).
//!
//! Runs a background thread with its own Wayland connection. Sends [`ToplevelEvent`]s
//! over an `mpsc` channel that the main GTK thread polls.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};

use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};

use wayland_protocols_plasma::plasma_window_management::client::{
    org_kde_plasma_window::{self, OrgKdePlasmaWindow},
    org_kde_plasma_window_management::{self, OrgKdePlasmaWindowManagement},
};

#[derive(Debug, Clone)]
pub enum ToplevelEvent {
    /// A new toplevel appeared. `has_parent` is `Some(true)` for KDE transient/dialog
    /// windows (have a parent), `Some(false)` for KDE regular apps, and `None` for the
    /// ext protocol which doesn't provide parent info.
    Added { has_parent: Option<bool> },
    Closed,
}

/// KDE `org_kde_plasma_window_management.state` flag for windows that should
/// not appear in the taskbar — notifications, tooltips, OSD popups, etc.
const KDE_STATE_SKIP_TASKBAR: u32 = 0x1000;

struct KdeWindowState {
    announced: bool,
    state_flags: u32,
    has_parent: bool,
}

pub struct ToplevelMonitor;

impl ToplevelMonitor {
    /// Attempt to start monitoring. Returns `None` if not running on Wayland
    /// or no supported toplevel protocol is available.
    pub fn start() -> Option<mpsc::Receiver<ToplevelEvent>> {
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<bool>(1);

        if thread::Builder::new()
            .name("toplevel-monitor".into())
            .spawn(move || run_monitor(tx, ready_tx))
            .is_err()
        {
            return None;
        }

        match ready_rx.recv() {
            Ok(true) => Some(rx),
            _ => None,
        }
    }
}

struct MonitorState {
    tx: mpsc::Sender<ToplevelEvent>,

    ext_global: Option<(u32, u32)>,
    kde_global: Option<(u32, u32)>,
    ext_list: Option<ExtForeignToplevelListV1>,
    kde_mgr: Option<OrgKdePlasmaWindowManagement>,

    /// `bool` = whether the handle has been announced via `Added`.
    ext_handles: HashMap<ExtForeignToplevelHandleV1, bool>,
    kde_windows: HashMap<OrgKdePlasmaWindow, KdeWindowState>,

    initial_done: bool,
}

fn run_monitor(tx: mpsc::Sender<ToplevelEvent>, ready_tx: mpsc::SyncSender<bool>) {
    let Ok(conn) = Connection::connect_to_env() else {
        let _ = ready_tx.send(false);
        return;
    };

    let display = conn.display();
    let mut event_queue = conn.new_event_queue::<MonitorState>();
    let qh = event_queue.handle();

    let mut state = MonitorState {
        tx,
        ext_global: None,
        kde_global: None,
        ext_list: None,
        kde_mgr: None,
        ext_handles: HashMap::new(),
        kde_windows: HashMap::new(),
        initial_done: false,
    };

    let registry = display.get_registry(&qh, ());

    if event_queue.roundtrip(&mut state).is_err() {
        let _ = ready_tx.send(false);
        return;
    }

    if let Some((name, version)) = state.ext_global {
        let list = registry.bind::<ExtForeignToplevelListV1, _, _>(
            name,
            version.min(1),
            &qh,
            (),
        );
        state.ext_list = Some(list);
    } else if let Some((name, version)) = state.kde_global {
        let mgr = registry.bind::<OrgKdePlasmaWindowManagement, _, _>(
            name,
            version.min(18),
            &qh,
            (),
        );
        state.kde_mgr = Some(mgr);
    } else {
        let _ = ready_tx.send(false);
        return;
    }

    // Receive pre-existing toplevels so we can ignore them.
    let _ = event_queue.roundtrip(&mut state);

    state.initial_done = true;

    let _ = ready_tx.send(true);

    loop {
        if event_queue.blocking_dispatch(&mut state).is_err() {
            break;
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for MonitorState {
    fn event(
        state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "ext_foreign_toplevel_list_v1" => {
                    state.ext_global = Some((name, version));
                }
                "org_kde_plasma_window_management" => {
                    state.kde_global = Some((name, version));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for MonitorState {
    fn event(
        state: &mut Self,
        _proxy: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.ext_handles.insert(toplevel, false);
        }
    }

    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        assert_eq!(opcode, 0, "unexpected child-creating opcode on ext_foreign_toplevel_list");
        qh.make_data::<ExtForeignToplevelHandleV1, ()>(())
    }
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for MonitorState {
    fn event(
        state: &mut Self,
        proxy: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_handle_v1::Event::Done => {
                let Some(announced) = state.ext_handles.get_mut(proxy) else { return };

                if *announced {
                    return;
                }

                *announced = true;

                if state.initial_done {
                    let _ = state.tx.send(ToplevelEvent::Added { has_parent: None });
                }
            }

            ext_foreign_toplevel_handle_v1::Event::Closed => {
                if let Some(true) = state.ext_handles.remove(proxy) {
                    let _ = state.tx.send(ToplevelEvent::Closed);
                }

                proxy.destroy();
            }

            _ => {}
        }
    }
}

impl Dispatch<OrgKdePlasmaWindowManagement, ()> for MonitorState {
    fn event(
        state: &mut Self,
        proxy: &OrgKdePlasmaWindowManagement,
        event: org_kde_plasma_window_management::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let org_kde_plasma_window_management::Event::WindowWithUuid { uuid, .. } = event {
            let window = proxy.get_window_by_uuid(uuid, qh, ());
            state.kde_windows.insert(window, KdeWindowState { announced: false, state_flags: 0, has_parent: false });
        }
    }
}

impl Dispatch<OrgKdePlasmaWindow, ()> for MonitorState {
    fn event(
        state: &mut Self,
        proxy: &OrgKdePlasmaWindow,
        event: org_kde_plasma_window::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            // State flags and parent arrive before InitialState, so we capture them first.
            org_kde_plasma_window::Event::StateChanged { flags } => {
                if let Some(win) = state.kde_windows.get_mut(proxy) {
                    win.state_flags = flags;
                }
            }

            org_kde_plasma_window::Event::ParentWindow { parent } => {
                if let Some(win) = state.kde_windows.get_mut(proxy) {
                    win.has_parent = parent.is_some();
                }
            }

            org_kde_plasma_window::Event::InitialState => {
                let Some(win) = state.kde_windows.get_mut(proxy) else { return };

                if win.announced {
                    return;
                }

                // Skip non-application windows (notifications, tooltips, OSD, etc.)
                if win.state_flags & KDE_STATE_SKIP_TASKBAR != 0 {
                    state.kde_windows.remove(proxy);
                    proxy.destroy();
                    return;
                }

                win.announced = true;

                if state.initial_done {
                    let _ = state.tx.send(ToplevelEvent::Added { has_parent: Some(win.has_parent) });
                }
            }

            org_kde_plasma_window::Event::Unmapped => {
                let was_announced = state.kde_windows.remove(proxy)
                    .is_some_and(|win| win.announced);

                if was_announced {
                    let _ = state.tx.send(ToplevelEvent::Closed);
                }

                proxy.destroy();
            }

            _ => {}
        }
    }
}

delegate_noop!(MonitorState: ignore wayland_client::protocol::wl_callback::WlCallback);
