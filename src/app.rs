use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use vte4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, EventControllerKey, Orientation, Paned,
    Stack, StackTransitionType,
    gdk::Key,
    glib,
};

use crate::claude;
use crate::config::{Config, SessionState};
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

    // Load config and saved session state
    let config = Rc::new(Config::load());
    let saved_state = SessionState::load();

    // Start hook server
    let hook_server = HookServer::new();
    let socket_path = hook_server.socket_path().clone();
    let hook_rx = hook_server.start();

    // Set up Claude wrapper scripts
    let bin_dir = claude::setup_scripts(&socket_path);

    // Layout: sidebar | drag handle | terminal stack (via GtkPaned)
    let sidebar = Rc::new(Sidebar::new());

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(StackTransitionType::None);

    let paned = Paned::new(Orientation::Horizontal);
    paned.set_start_child(Some(&sidebar.container));
    paned.set_end_child(Some(&stack));
    paned.set_position(saved_state.sidebar_width.unwrap_or(config.sidebar_width));
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);
    paned.set_resize_start_child(false);
    paned.set_resize_end_child(true);

    let manager = SessionManager::new(stack, sidebar.clone(), socket_path, bin_dir, config);

    // Notification store
    let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

    // Wire notification changes to sidebar badge + preview updates
    let sidebar_for_notif = sidebar.clone();
    notification_store.borrow_mut().set_on_change(move |session_id, count, latest| {
        sidebar_for_notif.update_badge(session_id, count);

        let preview = if count > 0 {
            latest.map(|n| n.body.as_str())
        } else {
            None
        };
        sidebar_for_notif.update_notification_preview(session_id, preview);
    });

    // Quit when all tabs are closed
    let app_clone = app.clone();
    manager.borrow_mut().set_on_empty(move || {
        app_clone.quit();
    });

    // Restore saved sessions or create a fresh one
    if saved_state.sessions.is_empty() {
        let first_id = manager.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(&sidebar, &manager, &first_id);
    } else {
        for saved in &saved_state.sessions {
            let cwd = saved.cwd.as_deref()
                .filter(|p| std::path::Path::new(p).exists());

            let id = manager.borrow_mut().create_session(Some(&saved.title), cwd);
            wire_tab_lifecycle(&sidebar, &manager, &id);
        }
    }

    // Poll hook events from the background thread
    let mgr_for_hooks = manager.clone();
    let notif_store = notification_store.clone();
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
    // CAPTURE phase so we see events before VTE consumes them
    key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let mgr = manager.clone();
    let sidebar_for_keys = sidebar.clone();
    let notif_for_keys = notification_store.clone();

    key_controller.connect_key_pressed(move |_, key, _keycode, modifiers| {
        let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gtk4::gdk::ModifierType::ALT_MASK);

        // Only intercept our specific shortcuts — let everything else through to VTE
        let is_our_shortcut = (ctrl && shift && matches!(key, Key::C | Key::V | Key::T | Key::W))
            || (ctrl && !shift && matches!(key, Key::t | Key::Tab))
            || (alt && matches!(key, Key::_1 | Key::_2 | Key::_3 | Key::_4 | Key::_5 | Key::_6 | Key::_7 | Key::_8 | Key::_9));

        if !is_our_shortcut {
            return glib::Propagation::Proceed;
        }

        // Ctrl+Shift+C: copy from terminal
        if ctrl && shift && key == Key::C {
            if let Some(term) = mgr.borrow().active_terminal() {
                term.terminal().copy_clipboard_format(vte4::Format::Text);
            }
            return glib::Propagation::Stop;
        }

        // Ctrl+Shift+V: paste to terminal
        if ctrl && shift && key == Key::V {
            if let Some(term) = mgr.borrow().active_terminal() {
                term.terminal().paste_clipboard();
            }
            return glib::Propagation::Stop;
        }

        // Ctrl+T or Ctrl+Shift+T: new tab
        if ctrl && (key == Key::t || key == Key::T) {
            let id = mgr.borrow_mut().create_session(None, None);
            wire_tab_lifecycle(&sidebar_for_keys, &mgr, &id);
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

    // Save session state on window close
    let mgr_for_close = manager.clone();
    let paned_for_close = paned.clone();
    window.connect_close_request(move |_| {
        mgr_for_close.borrow().save_state(paned_for_close.position());
        glib::Propagation::Proceed
    });

    // Keep hook_server alive for the window's lifetime.
    let _hook_server: &'static _ = Box::leak(Box::new(hook_server));

    window.set_child(Some(&paned));
    window.present();

    // Spawn deferred shells once the window is mapped and terminals have their real size
    let mgr_for_map = manager.clone();
    glib::idle_add_local_once(move || {
        mgr_for_map.borrow().spawn_deferred();
    });
}

fn wire_tab_lifecycle(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    session_id: &str,
) {
    let mgr = manager.clone();
    sidebar.wire_close_button(session_id, move |id| {
        mgr.borrow_mut().destroy_session(&id);
    });

    let sidebar_rename = sidebar.clone();
    sidebar.wire_rename(session_id, move |id, new_title| {
        sidebar_rename.update_title(&id, &new_title);
    });

    SessionManager::wire_child_exited(manager, session_id);
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
