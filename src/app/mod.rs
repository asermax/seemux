mod actions;
mod dialogs;
mod hooks;
mod keyboard;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, EventControllerKey, Orientation,
    Overlay, Paned, Stack, StackTransitionType,
    gio,
    gdk::Key,
    glib,
};

use crate::app_state::AppState;
use crate::config::SessionState;
use crate::notifications::NotificationStore;
use crate::persistence::StatePersistence;
use crate::session::manager::{self, SessionManager};
use crate::sidebar::Sidebar;
use crate::sidebar::collapsed_bar::COLLAPSED_WIDTH;
use crate::tray::TrayHandle;

pub fn build_window(app: &Application, state: &Rc<AppState>) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("seemux")
        .default_width(1000)
        .default_height(700)
        .build();

    let config = state.config.clone();
    let socket_path = state.socket_path.clone();

    // Layout: sidebar | drag handle | terminal stack (via GtkPaned)
    let scheme = crate::theme::get_scheme(&config.borrow().color_scheme);

    let tray_handle = if config.borrow().tray_enabled {
        crate::tray::setup_tray(&config.borrow().tray_icon, &socket_path, false, scheme.accent)
    } else {
        crate::tray::TrayHandle::disabled()
    };
    let sidebar = Rc::new(Sidebar::new(scheme));

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(StackTransitionType::None);

    let paned = Paned::new(Orientation::Horizontal);
    paned.set_start_child(Some(&sidebar.container));
    paned.set_end_child(Some(&stack));
    paned.set_position(config.borrow().sidebar_width);
    paned.set_wide_handle(true);
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);
    paned.set_resize_start_child(false);
    paned.set_resize_end_child(true);

    wire_sidebar_collapse(&sidebar, &paned, &config);

    let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

    let manager = SessionManager::new(
        stack.clone(), sidebar.clone(), socket_path, config.clone(), notification_store.clone(),
    );

    // Wrap content in overlay for centered dialogs
    let overlay = Overlay::new();
    overlay.set_child(Some(&paned));

    let persistence = StatePersistence::new(
        manager.clone(), paned.clone(), config.clone(), sidebar.clone(),
    );

    // Common setup: actions, context menus, notification wiring, DnD, signal handlers
    setup_common(&window, &manager, &sidebar, &notification_store, &stack, &persistence, &tray_handle);

    // Quit when all tabs are closed
    let app_clone = app.clone();
    manager.borrow_mut().set_on_empty(move || {
        app_clone.quit();
    });

    // Restore saved sessions/groups or create a fresh tab
    restore_sessions(&sidebar, &manager, &notification_store);
    wire_new_tab_button(&sidebar, &manager, &notification_store);

    // Restore collapsed state after sessions are loaded so rebuild sees all tabs
    restore_sidebar_collapsed(&sidebar, &paned, &config);

    // Wire state-changed callback *after* restore to avoid saving during load
    let p = persistence.clone();
    manager.borrow_mut().set_on_state_changed(move || p.mark_dirty());

    // Shared "create new group" logic — used by both sidebar button and Ctrl+Shift+G
    let create_group = make_create_group_action(
        &manager, &sidebar, &notification_store, &overlay,
    );

    // Wire "New Group" sidebar button
    let create_group_btn = create_group.clone();
    sidebar.connect_new_group(move || create_group_btn());

    // Create dropdown window (shown via `seemux toggle` CLI command)
    let dropdown = Rc::new(crate::dropdown::DropdownWindow::new(app, state));

    hooks::setup_hook_polling(state, &manager, &notification_store, &sidebar, Some(dropdown), window.clone());
    hooks::setup_stale_pid_detection(&manager);

    // Keyboard shortcuts
    let on_new_tab = make_new_tab_action(&manager, &sidebar, &notification_store);

    let window_ref = window.clone();
    let create_group_key = create_group.clone();
    #[allow(clippy::type_complexity)]
    let extra_handler: Option<Rc<dyn Fn(Key, bool, bool) -> Option<glib::Propagation>>> = Some(Rc::new(move |key, ctrl, shift| {
        if ctrl && shift && key == Key::N {
            if let Some(app) = window_ref.application() {
                app.activate();
            }
            return Some(glib::Propagation::Stop);
        }

        if ctrl && shift && key == Key::G {
            create_group_key();
            return Some(glib::Propagation::Stop);
        }

        None
    }));

    keyboard::setup_keyboard_shortcuts(&window, &manager, &sidebar, &notification_store, on_new_tab, extra_handler);

    // Hide tab-index overlays when the window loses focus — the Alt key
    // release event is swallowed by the window manager during Alt+Tab.
    let sidebar_for_focus = sidebar.clone();
    window.connect_notify_local(Some("is-active"), move |window, _| {
        if !window.is_active() {
            sidebar_for_focus.hide_tab_indices();
        }
    });

    // Save session state and sidebar width on window close
    let persistence_for_close = persistence.clone();
    let app_for_close = app.clone();
    let tray_for_close = tray_handle.clone();
    window.connect_close_request(move |_| {
        tray_for_close.shutdown();
        persistence_for_close.save_now();
        app_for_close.quit();
        glib::Propagation::Proceed
    });

    window.set_child(Some(&overlay));
    window.present();

    schedule_deferred_spawn(&manager, true);
}

pub fn build_quake_window(app: &Application, state: &Rc<AppState>) {
    let scheme = crate::theme::get_scheme(&state.config.borrow().color_scheme);

    let tray_handle = if state.config.borrow().tray_enabled {
        crate::tray::setup_tray(&state.config.borrow().tray_icon, &state.socket_path, true, scheme.accent)
    } else {
        crate::tray::TrayHandle::disabled()
    };

    let dropdown = Rc::new(crate::dropdown::DropdownWindow::new(app, state));

    wire_sidebar_collapse(&dropdown.sidebar, &dropdown.paned, &state.config);

    hooks::setup_hook_polling(state, &dropdown.manager, &dropdown.notification_store, &dropdown.sidebar, Some(dropdown.clone()), dropdown.window().clone());
    hooks::setup_stale_pid_detection(&dropdown.manager);

    let persistence = StatePersistence::new(
        dropdown.manager.clone(), dropdown.paned.clone(), state.config.clone(),
        dropdown.sidebar.clone(),
    );

    // Common setup: actions, context menus, notification wiring, DnD, signal handlers
    setup_common(
        dropdown.window(),
        &dropdown.manager,
        &dropdown.sidebar,
        &dropdown.notification_store,
        &dropdown.stack,
        &persistence,
        &tray_handle,
    );

    // Register hide-dropdown action so open-url can dismiss the dropdown
    let dd = dropdown.clone();
    let hide_action = gio::SimpleAction::new("hide-dropdown", None);
    hide_action.connect_activate(move |_, _| { dd.hide(); });
    dropdown.window().add_action(&hide_action);

    // When all sessions are closed, respawn a new one
    let mgr_empty = dropdown.manager.clone();
    let sid_empty = dropdown.sidebar.clone();
    let notif_empty = dropdown.notification_store.clone();
    dropdown.manager.borrow_mut().set_on_empty(move || {
        let mgr = mgr_empty.clone();
        let sid = sid_empty.clone();
        let notif = notif_empty.clone();

        glib::idle_add_local_once(move || {
            let id = mgr.borrow_mut().create_session(None, None);
            wire_tab_lifecycle(&sid, &mgr, &notif, &id);
            mgr.borrow().spawn_deferred();
        });
    });

    // Restore saved sessions/groups or create a fresh tab
    restore_sessions(&dropdown.sidebar, &dropdown.manager, &dropdown.notification_store);
    wire_new_tab_button(&dropdown.sidebar, &dropdown.manager, &dropdown.notification_store);

    // Restore collapsed state after sessions are loaded so rebuild sees all tabs
    restore_sidebar_collapsed(&dropdown.sidebar, &dropdown.paned, &state.config);

    // Wire state-changed callback *after* restore to avoid saving during load
    let p = persistence.clone();
    dropdown.manager.borrow_mut().set_on_state_changed(move || p.mark_dirty());

    // Shared "create new group" logic — Ctrl+Shift+G and sidebar button
    let create_group = make_create_group_action(
        &dropdown.manager, &dropdown.sidebar, &dropdown.notification_store, &dropdown.overlay,
    );

    let create_group_btn = create_group.clone();
    dropdown.sidebar.connect_new_group(move || create_group_btn());

    // Keyboard shortcuts
    let on_new_tab = make_new_tab_action(&dropdown.manager, &dropdown.sidebar, &dropdown.notification_store);

    #[allow(clippy::type_complexity)]
    let extra_handler: Option<Rc<dyn Fn(Key, bool, bool) -> Option<glib::Propagation>>> = {
        let create_group_key = create_group.clone();

        Some(Rc::new(move |key, ctrl, shift| {
            if ctrl && shift && key == Key::G {
                create_group_key();
                return Some(glib::Propagation::Stop);
            }

            None
        }))
    };

    keyboard::setup_keyboard_shortcuts(
        dropdown.window(),
        &dropdown.manager,
        &dropdown.sidebar,
        &dropdown.notification_store,
        on_new_tab,
        extra_handler,
    );

    // Track non-modifier keypresses so we can distinguish spurious focus
    // loss (e.g. wl-copy briefly stealing focus) from intentional switches.
    let dropdown_for_kp = dropdown.clone();
    let kp_controller = EventControllerKey::new();
    kp_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

    kp_controller.connect_key_pressed(move |_, key, _, _| {
        let is_modifier = matches!(key,
            Key::Shift_L | Key::Shift_R |
            Key::Control_L | Key::Control_R |
            Key::Alt_L | Key::Alt_R |
            Key::Super_L | Key::Super_R |
            Key::Meta_L | Key::Meta_R
        );

        if !is_modifier {
            dropdown_for_kp.record_keypress();
        }

        glib::Propagation::Proceed
    });

    dropdown.window().add_controller(kp_controller);

    // Auto-hide when another window gets focus.
    // Use a short delay to avoid hiding when a popover (context menu) or
    // clipboard tool (wl-copy) briefly steals focus.
    // If there was recent keyboard activity the focus loss is likely
    // spurious, so we try to recover via present() first.
    let hide_generation: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0));
    let dropdown_for_focus = dropdown.clone();
    let hide_gen = hide_generation.clone();
    dropdown.window().connect_notify_local(Some("is-active"), move |window, _| {
        if !window.is_active() && *dropdown_for_focus.visible() {
            if dropdown_for_focus.had_recent_keypress() {
                window.present();
            }

            let current = hide_gen.get().wrapping_add(1);
            hide_gen.set(current);

            let dd = dropdown_for_focus.clone();
            let gen_check = hide_gen.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                if gen_check.get() == current && *dd.visible() && !dd.window().is_active() {
                    dd.hide();
                }
            });
        } else {
            // Window became active again — cancel any pending hide
            hide_gen.set(hide_gen.get().wrapping_add(1));
        }
    });

    // Save session state on close
    let persistence_for_close = persistence.clone();
    let tray_for_close = tray_handle.clone();
    dropdown.window().connect_close_request(move |_| {
        tray_for_close.shutdown();
        persistence_for_close.save_now();
        glib::Propagation::Proceed
    });

    // Register global shortcut via XDG Portal (best-effort)
    let dropdown_for_shortcut = dropdown.clone();
    crate::global_shortcuts::register_toggle(move || dropdown_for_shortcut.toggle());

    schedule_deferred_spawn(&dropdown.manager, false);

    // Present the window off-screen, ready for the first toggle
    dropdown.present_hidden();
}

// --- Shared helpers ---

/// Common setup shared by both normal and quake windows: context menu actions,
/// terminal right-click, notification badge wiring, and DnD tab reordering.
fn setup_common(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    stack: &Stack,
    persistence: &Rc<StatePersistence>,
    tray_handle: &TrayHandle,
) {
    actions::register_tab_actions(window, manager, sidebar, persistence);
    actions::register_terminal_actions(window, manager, sidebar, notification_store);
    actions::setup_terminal_context_menu(stack);
    actions::setup_ctrl_click_url_open(stack);

    // Wire notification changes to sidebar badge + preview updates + peek + tray
    let sidebar_for_notif = sidebar.clone();
    let tray = tray_handle.clone();
    notification_store.borrow_mut().set_on_change(move |session_id, count, latest, total| {
        sidebar_for_notif.update_badge(session_id, count);

        let preview = if count > 0 {
            latest.map(|n| n.body.as_str())
        } else {
            None
        };
        sidebar_for_notif.update_notification_preview(session_id, preview);

        if count > 0 {
            sidebar_for_notif.peek_tab(session_id);
        } else {
            sidebar_for_notif.unpeek_tab(session_id);
        }

        tray.update_count(total);
    });

    // Wire drag-and-drop tab movement/reordering
    let mgr_for_dnd = manager.clone();
    sidebar.set_on_tab_moved(move |session_id, new_group, position| {
        mgr_for_dnd.borrow_mut().move_session_to_position(&session_id, &new_group, position);
    });

    // Wire collapsed bar dot clicks to switch tab + mark notifications read
    let mgr_for_dot = manager.clone();
    let notif_for_dot = notification_store.clone();
    sidebar.collapsed_bar().set_on_dot_click(move |session_id| {
        mgr_for_dot.borrow_mut().switch_to(&session_id);
        notif_for_dot.borrow_mut().mark_read(&session_id);
    });

    // Poll signal flags to save state on SIGTERM / SIGHUP
    setup_signal_save(persistence);
}

/// Register signal handlers for SIGTERM and SIGHUP that save state before exit.
///
/// Uses AtomicBool flags polled from the GTK main loop (100ms) since glib 0.22
/// doesn't expose `g_unix_signal_add` as a safe Rust wrapper.
fn setup_signal_save(persistence: &Rc<StatePersistence>) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);

    // Install signal handlers (safe: only sets an atomic flag)
    unsafe {
        libc::signal(libc::SIGTERM, signal_handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, signal_handler as *const () as libc::sighandler_t);
    }

    extern "C" fn signal_handler(_sig: libc::c_int) {
        SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
    }

    // Poll the flag from the GTK main loop
    let p = persistence.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        if SIGNAL_RECEIVED.load(Ordering::Relaxed) {
            p.save_now();

            if let Some(app) = gio::Application::default() {
                app.quit();
            }

            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });
}

/// Wire sidebar collapse/expand to paned position management.
/// Stores the expanded width in the sidebar and sets up the on_collapse_changed callback.
/// Does NOT restore collapsed state from config — that must happen after session restore.
fn wire_sidebar_collapse(
    sidebar: &Rc<Sidebar>,
    paned: &Paned,
    config: &Rc<RefCell<crate::config::Config>>,
) {
    sidebar.expanded_width.set(config.borrow().sidebar_width);

    // Lock flag: when true, snap paned position back to COLLAPSED_WIDTH
    let locked = Rc::new(Cell::new(false));
    let snapping = Rc::new(Cell::new(false));

    let paned_for_collapse = paned.clone();
    let sidebar_for_collapse = sidebar.clone();
    let locked_for_collapse = locked.clone();
    sidebar.set_on_collapse_changed(move |collapsed| {
        locked_for_collapse.set(collapsed);

        if collapsed {
            sidebar_for_collapse.expanded_width.set(paned_for_collapse.position());
            paned_for_collapse.set_position(COLLAPSED_WIDTH);
            paned_for_collapse.set_wide_handle(false);
            paned_for_collapse.add_css_class("sidebar-locked");
        } else {
            paned_for_collapse.set_position(sidebar_for_collapse.expanded_width.get());
            paned_for_collapse.set_wide_handle(true);
            paned_for_collapse.remove_css_class("sidebar-locked");
        }
    });

    // Snap-back guard: prevent dragging the separator when collapsed
    let locked_for_notify = locked.clone();
    let snapping_for_notify = snapping.clone();
    paned.connect_notify_local(Some("position"), move |paned, _| {
        if locked_for_notify.get() && !snapping_for_notify.get() && paned.position() != COLLAPSED_WIDTH {
            snapping_for_notify.set(true);
            paned.set_position(COLLAPSED_WIDTH);
            snapping_for_notify.set(false);
        }
    });

    // Refresh collapsed bar when tab group visibility changes
    let sidebar_for_group = sidebar.clone();
    sidebar.set_on_group_visibility_changed(move || {
        sidebar_for_group.refresh_collapsed_bar();
    });
}

/// Restore collapsed sidebar state from config. Must be called after sessions are restored
/// so the collapsed bar's rebuild sees all tabs.
fn restore_sidebar_collapsed(
    sidebar: &Rc<Sidebar>,
    paned: &Paned,
    config: &Rc<RefCell<crate::config::Config>>,
) {
    if config.borrow().sidebar_collapsed {
        sidebar.set_sidebar_collapsed(true);
        paned.set_position(COLLAPSED_WIDTH);
    }
}

pub(crate) fn wire_tab_lifecycle(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    session_id: &str,
) {
    // Click to select + auto-read notifications
    let mgr = manager.clone();
    let notif = notification_store.clone();
    sidebar.wire_tab_click(session_id, move |id| {
        if let Ok(mut m) = mgr.try_borrow_mut() {
            m.switch_to(&id);
        }
        notif.borrow_mut().mark_read(&id);
    });

    let mgr = manager.clone();
    sidebar.wire_close_button(session_id, move |id| {
        mgr.borrow_mut().destroy_session(&id);
        refocus_terminal(&mgr);
    });

    sidebar.setup_context_menu(session_id);
    SessionManager::wire_pane_signals(manager, session_id);
}

/// Restore saved groups and sessions from disk, or create a fresh tab if none exist.
fn restore_sessions(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) {
    let saved_state = SessionState::load();

    for group in &saved_state.groups {
        register_group(&group.id, &group.name, sidebar, manager, notification_store);

        if group.collapsed {
            sidebar.collapse_group(&group.id);
        }
    }

    if saved_state.sessions.is_empty() {
        let first_id = manager.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(sidebar, manager, notification_store, &first_id);
    } else {
        for saved in &saved_state.sessions {
            let group = if saved.group_id.is_empty() { crate::session::DEFAULT_GROUP } else { &saved.group_id };
            let id = manager.borrow_mut().restore_session_with_splits(
                &saved.title,
                group,
                &saved.split_tree,
                saved.claude_session_id.as_deref(),
            );
            wire_tab_lifecycle(sidebar, manager, notification_store, &id);
        }

        if let Some(idx) = saved_state.active_session_index {
            manager.borrow_mut().switch_to_index(idx);
        }
    }
}

/// Wire default group's "+ Add tab" button.
fn wire_new_tab_button(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) {
    let mgr = manager.clone();
    let sid = sidebar.clone();
    let notif = notification_store.clone();
    sidebar.connect_new_tab(move || {
        let id = mgr.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(&sid, &mgr, &notif, &id);
    });
}

/// Build the closure for keyboard shortcut Ctrl+T that creates a new tab
/// in the active group, inheriting the active terminal's CWD.
fn make_new_tab_action(
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) -> Rc<dyn Fn()> {
    let mgr = manager.clone();
    let sidebar = sidebar.clone();
    let notif = notification_store.clone();

    Rc::new(move || {
        let mgr_ref = mgr.borrow();

        let cwd = mgr_ref
            .active_terminal_vte()
            .and_then(|term| term.current_directory_uri())
            .and_then(|uri| manager::path_from_file_uri(&uri));

        let group_id = mgr_ref.active_group_id()
            .unwrap_or(crate::session::DEFAULT_GROUP)
            .to_string();

        drop(mgr_ref);

        let id = mgr.borrow_mut().create_session_in_group(None, cwd.as_deref(), &group_id);
        wire_tab_lifecycle(&sidebar, &mgr, &notif, &id);
    })
}

/// Build a closure that shows the "new group" overlay and wires the new group's
/// tab lifecycle. Used by both normal and quake windows.
fn make_create_group_action(
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    overlay: &Overlay,
) -> Rc<dyn Fn()> {
    let mgr = manager.clone();
    let sid = sidebar.clone();
    let notif = notification_store.clone();
    let overlay = overlay.clone();

    Rc::new(move || {
        let mgr = mgr.clone();
        let sid = sid.clone();
        let notif = notif.clone();

        let mgr_for_overlay = mgr.clone();
        dialogs::show_new_group_overlay(&overlay, &mgr_for_overlay, move |name| {
            let group_id = create_group_programmatic(&name, &sid, &mgr, &notif);

            let first_id = mgr.borrow_mut().create_session_in_group(None, None, &group_id);
            wire_tab_lifecycle(&sid, &mgr, &notif, &first_id);
        });
    })
}

pub(crate) fn refocus_terminal(manager: &Rc<RefCell<SessionManager>>) {
    if let Some(term) = manager.borrow().active_terminal_vte() {
        term.grab_focus();
    }
}

/// Create a named group programmatically (without the overlay dialog).
/// Wires the group's "new tab" button and returns the group ID.
pub(crate) fn create_group_programmatic(
    name: &str,
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) -> String {
    let group_id = uuid::Uuid::new_v4().to_string();
    register_group(&group_id, name, sidebar, manager, notification_store);
    group_id
}

/// Register a group with a known ID and wire its "new tab" button.
/// Used by both programmatic creation and session restoration.
fn register_group(
    group_id: &str,
    name: &str,
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) {
    sidebar.add_group(group_id, name);

    let mgr = manager.clone();
    let sid = sidebar.clone();
    let notif = notification_store.clone();
    let gid = group_id.to_string();
    let sid_expand = sidebar.clone();
    let gid_expand = group_id.to_string();
    sidebar.connect_group_new_tab(group_id, move |_| {
        sid_expand.expand_group(&gid_expand);
        let id = mgr.borrow_mut().create_session_in_group(None, None, &gid);
        wire_tab_lifecycle(&sid, &mgr, &notif, &id);
    });
}

/// Spawn deferred shells and resume any Claude sessions that were active at shutdown.
fn schedule_deferred_spawn(manager: &Rc<RefCell<SessionManager>>, grab_focus: bool) {
    let mgr = manager.clone();

    glib::idle_add_local_once(move || {
        let pending = mgr.borrow().sessions_pending_resume();
        mgr.borrow().spawn_deferred();

        if grab_focus
            && let Some(term) = mgr.borrow().active_terminal_vte() {
                term.grab_focus();
        }

        if !pending.is_empty() {
            let mgr = mgr.clone();

            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                for (session_id, claude_session_id, collapsed) in &pending {
                    if let Some(term) = mgr.borrow().session_terminal(session_id) {
                        if *collapsed {
                            term.feed_child(format!("claude --resume {claude_session_id}").as_bytes());
                        } else {
                            term.feed_child(format!("claude --resume {claude_session_id}\n").as_bytes());
                        }
                    }
                }
            });
        }
    });
}
