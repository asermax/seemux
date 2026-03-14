use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, EventControllerKey, Orientation, Separator,
    Stack, StackTransitionType,
    gdk::Key,
    glib,
};

use crate::claude;
use crate::notifications::hook_handler;
use crate::notifications::hook_server::HookServer;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

pub fn build_window(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("seemux")
        .default_width(1000)
        .default_height(700)
        .build();

    // Start hook server
    let hook_server = HookServer::new();
    let socket_path = hook_server.socket_path().clone();
    let hook_rx = hook_server.start();

    // Set up Claude wrapper scripts
    let bin_dir = claude::setup_scripts(&socket_path);

    // Layout: sidebar | separator | terminal stack
    let root = GtkBox::new(Orientation::Horizontal, 0);

    let sidebar = Rc::new(Sidebar::new());

    let separator = Separator::new(Orientation::Vertical);

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(StackTransitionType::None);

    root.append(&sidebar.container);
    root.append(&separator);
    root.append(&stack);

    let manager = SessionManager::new(stack, sidebar.clone(), socket_path, bin_dir);

    // Notification store
    let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

    // Wire notification changes to sidebar badge updates
    let sidebar_for_notif = sidebar.clone();
    notification_store.borrow_mut().set_on_change(move |session_id, count, _latest| {
        sidebar_for_notif.update_badge(session_id, count);
    });

    // Quit when all tabs are closed
    let app_clone = app.clone();
    manager.borrow_mut().set_on_empty(move || {
        app_clone.quit();
    });

    // Create the first session
    let first_id = manager.borrow_mut().create_session(None);
    wire_close_button(&sidebar, &manager, &first_id);

    // Poll hook events from the background thread
    let mgr_for_hooks = manager.clone();
    let notif_store = notification_store.clone();
    let _sidebar_for_hooks = sidebar.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(event) = hook_rx.try_recv() {
            let result = hook_handler::handle_hook_event(event);

            // Update session status
            if let Some(status) = result.new_status {
                mgr_for_hooks.borrow_mut().update_session_status(&result.session_id, status);
            }

            // Update Claude PID
            if let Some(pid) = result.claude_pid {
                let pid_val = if pid == 0 { None } else { Some(pid) };
                mgr_for_hooks.borrow_mut().set_claude_pid(&result.session_id, pid_val);
            }

            // Clear notifications if requested
            if result.clear_notifications {
                notif_store.borrow_mut().clear_session(&result.session_id);
            }

            // Add notification if present
            if let Some((title, subtitle, body)) = result.notification {
                let notification = crate::notifications::Notification::new(
                    &result.session_id,
                    &title,
                    &subtitle,
                    &body,
                );

                // Send desktop notification if tab is not active
                let is_active = mgr_for_hooks.borrow().active_id()
                    .map(|id| id == result.session_id)
                    .unwrap_or(false);

                if !is_active && matches!(subtitle.as_str(), "Permission" | "Error" | "Waiting" | "Attention") {
                    send_desktop_notification(&title, &subtitle, &body);
                }

                notif_store.borrow_mut().add_notification(notification);
            }
        }

        glib::ControlFlow::Continue
    });

    // Stale PID detection — check every 5 seconds
    let mgr_for_pid = manager.clone();
    glib::timeout_add_seconds_local(5, move || {
        let sessions = mgr_for_pid.borrow().sessions_with_claude_pid();

        for (session_id, pid) in sessions {
            let alive = unsafe { libc::kill(pid as i32, 0) } == 0;

            if !alive {
                mgr_for_pid.borrow_mut().set_claude_pid(&session_id, None);
                mgr_for_pid.borrow_mut().update_session_status(
                    &session_id,
                    crate::session::SessionStatus::Idle,
                );
            }
        }

        glib::ControlFlow::Continue
    });

    // Keyboard shortcuts
    let key_controller = EventControllerKey::new();
    let mgr = manager.clone();
    let sidebar_for_keys = sidebar.clone();
    let notif_for_keys = notification_store.clone();

    key_controller.connect_key_pressed(move |_, key, _keycode, modifiers| {
        let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gtk4::gdk::ModifierType::ALT_MASK);

        // Ctrl+Shift+T: new tab
        if ctrl && shift && key == Key::T {
            let id = mgr.borrow_mut().create_session(None);
            wire_close_button(&sidebar_for_keys, &mgr, &id);
            return glib::Propagation::Stop;
        }

        // Ctrl+Shift+W: close current tab
        if ctrl && shift && key == Key::W {
            let active = mgr.borrow().active_id().map(|s| s.to_string());

            if let Some(id) = active {
                mgr.borrow_mut().destroy_session(&id);
            }

            return glib::Propagation::Stop;
        }

        // Alt+1-9: switch to tab by index
        if alt {
            let tab_index = match key {
                Key::_1 => Some(0),
                Key::_2 => Some(1),
                Key::_3 => Some(2),
                Key::_4 => Some(3),
                Key::_5 => Some(4),
                Key::_6 => Some(5),
                Key::_7 => Some(6),
                Key::_8 => Some(7),
                Key::_9 => Some(8),
                _ => None,
            };

            if let Some(idx) = tab_index {
                mgr.borrow_mut().switch_to_index(idx);

                // Auto-mark notifications as read for switched tab
                if let Some(active) = mgr.borrow().active_id() {
                    notif_for_keys.borrow_mut().mark_read(active);
                }

                return glib::Propagation::Stop;
            }
        }

        // Ctrl+Tab / Ctrl+Shift+Tab: cycle tabs
        if ctrl && key == Key::Tab {
            if shift {
                mgr.borrow_mut().switch_prev();
            } else {
                mgr.borrow_mut().switch_next();
            }

            // Auto-mark notifications as read
            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        glib::Propagation::Proceed
    });

    window.add_controller(key_controller);

    // Auto-mark notifications as read on sidebar tab click
    let mgr_for_sidebar = manager.clone();
    let notif_for_sidebar = notification_store.clone();
    sidebar.connect_tab_selected(move |_id| {
        if let Some(active) = mgr_for_sidebar.borrow().active_id() {
            notif_for_sidebar.borrow_mut().mark_read(active);
        }
    });

    // Keep hook_server alive for the window's lifetime.
    // Leak into a static ref — cleaned up on process exit via Drop.
    let _hook_server: &'static _ = Box::leak(Box::new(hook_server));

    window.set_child(Some(&root));
    window.present();
}

fn wire_close_button(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<crate::session::manager::SessionManager>>,
    session_id: &str,
) {
    let mgr = manager.clone();
    sidebar.wire_close_button(session_id, move |id| {
        mgr.borrow_mut().destroy_session(&id);
    });
}

fn send_desktop_notification(title: &str, subtitle: &str, body: &str) {
    let summary = format!("{title} — {subtitle}");

    if let Err(e) = notify_rust::Notification::new()
        .summary(&summary)
        .body(body)
        .timeout(5000)
        .show()
    {
        eprintln!("Desktop notification failed: {e}");
    }
}
