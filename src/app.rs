use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use vte4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, EventControllerKey, GestureClick, Orientation,
    Paned, PopoverMenu, Stack, StackTransitionType,
    gio,
    gdk::Key,
    glib,
};

use crate::app_state::AppState;
use crate::config::SessionState;
use crate::notifications::hook_handler;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

pub fn build_window(app: &Application, state: &Rc<AppState>) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("seemux")
        .default_width(1000)
        .default_height(700)
        .build();

    let config = state.config.clone();
    let saved_state = SessionState::load();
    let socket_path = state.socket_path.clone();

    // Layout: sidebar | drag handle | terminal stack (via GtkPaned)
    let sidebar = Rc::new(Sidebar::new());

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

    let manager = SessionManager::new(stack.clone(), sidebar.clone(), socket_path, config.clone());

    // Wire drag-and-drop tab movement between groups
    let mgr_for_dnd = manager.clone();
    sidebar.set_on_tab_moved(move |session_id, new_group| {
        mgr_for_dnd.borrow_mut().move_session_to_group(&session_id, &new_group);
    });

    // Register window actions for context menus
    register_tab_actions(&window, &manager, &sidebar);
    register_terminal_actions(&window, &manager);
    setup_terminal_context_menu(&stack);

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

    // Restore saved groups first
    for group in &saved_state.groups {
        sidebar.add_group(&group.id, &group.name);

        let mgr = manager.clone();
        let sid = sidebar.clone();
        let notif = notification_store.clone();
        let gid = group.id.clone();
        let sid_expand = sidebar.clone();
        let gid_expand = group.id.clone();
        sidebar.connect_group_new_tab(&group.id, move |_| {
            sid_expand.expand_group(&gid_expand);
            let id = mgr.borrow_mut().create_session_in_group(None, None, &gid);
            wire_tab_lifecycle(&sid, &mgr, &notif, &id);
        });
    }

    // Restore saved sessions or create a fresh one
    if saved_state.sessions.is_empty() {
        let first_id = manager.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(&sidebar, &manager, &notification_store, &first_id);
    } else {
        for saved in &saved_state.sessions {
            let group = if saved.group_id.is_empty() { crate::session::DEFAULT_GROUP } else { &saved.group_id };
            let id = manager.borrow_mut().restore_session_with_splits(
                &saved.title,
                group,
                &saved.split_tree,
            );
            wire_tab_lifecycle(&sidebar, &manager, &notification_store, &id);
        }

        // Restore last focused tab
        if let Some(idx) = saved_state.active_session_index {
            manager.borrow_mut().switch_to_index(idx);
        }
    }

    // Wire default group's "+ Add tab" button
    let mgr_new_tab = manager.clone();
    let sidebar_new_tab = sidebar.clone();
    let notif_new_tab = notification_store.clone();
    sidebar.connect_new_tab(move || {
        let id = mgr_new_tab.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(&sidebar_new_tab, &mgr_new_tab, &notif_new_tab, &id);
    });

    // Shared "create new group" logic — used by both sidebar button and Ctrl+Shift+G
    let create_group = {
        let mgr = manager.clone();
        let sid = sidebar.clone();
        let notif = notification_store.clone();
        let win = window.clone();

        Rc::new(move || {
            let mgr = mgr.clone();
            let sid = sid.clone();
            let notif = notif.clone();

            show_new_group_dialog(&win, move |name| {
                let group_id = uuid::Uuid::new_v4().to_string();
                sid.add_group(&group_id, &name);

                // Wire the group's "+" button
                let mgr2 = mgr.clone();
                let sid2 = sid.clone();
                let notif2 = notif.clone();
                let gid = group_id.clone();
                let sid_expand = sid.clone();
                let gid_expand = group_id.clone();
                sid.connect_group_new_tab(&group_id, move |_| {
                    sid_expand.expand_group(&gid_expand);
                    let id = mgr2.borrow_mut().create_session_in_group(None, None, &gid);
                    wire_tab_lifecycle(&sid2, &mgr2, &notif2, &id);
                });

                // Create initial tab in the new group
                let first_id = mgr.borrow_mut().create_session_in_group(None, None, &group_id);
                wire_tab_lifecycle(&sid, &mgr, &notif, &first_id);
            });
        })
    };

    // Wire "New Group" sidebar button
    let create_group_btn = create_group.clone();
    sidebar.connect_new_group(move || create_group_btn());

    // Create dropdown window (shown via `seemux toggle` CLI command)
    let dropdown = Rc::new(crate::dropdown::DropdownWindow::new(app, state));

    setup_hook_polling(state, &manager, &notification_store, Some(dropdown));
    setup_stale_pid_detection(&manager);

    // Keyboard shortcuts
    let on_new_tab: Rc<dyn Fn()> = {
        let sidebar = sidebar.clone();
        let mgr = manager.clone();
        let notif = notification_store.clone();

        Rc::new(move || {
            let id = mgr.borrow_mut().create_session(None, None);
            wire_tab_lifecycle(&sidebar, &mgr, &notif, &id);
        })
    };

    let window_ref = window.clone();
    let create_group_key = create_group.clone();
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

    setup_keyboard_shortcuts(&window, &manager, &notification_store, on_new_tab, extra_handler);

    // Note: auto-read on tab click is handled in wire_tab_lifecycle via wire_tab_click

    // Save session state and sidebar width on window close
    let mgr_for_close = manager.clone();
    let paned_for_close = paned.clone();
    let config_for_close = config.clone();
    let app_for_close = app.clone();
    window.connect_close_request(move |_| {
        mgr_for_close.borrow().save_state();

        let mut cfg = config_for_close.borrow_mut();
        cfg.sidebar_width = paned_for_close.position();
        cfg.save();

        // Quit the app — daemon threads will be killed on process exit
        app_for_close.quit();
        glib::Propagation::Proceed
    });

    window.set_child(Some(&paned));
    window.present();

    // Spawn deferred shells and focus the first terminal
    let mgr_for_map = manager.clone();
    glib::idle_add_local_once(move || {
        mgr_for_map.borrow().spawn_deferred();

        // Focus the active terminal
        if let Some(term) = mgr_for_map.borrow().active_terminal_vte() {
            term.grab_focus();
        }
    });
}

fn wire_tab_lifecycle(
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
    });

    let sidebar_rename = sidebar.clone();
    sidebar.wire_rename(session_id, move |id, new_title| {
        sidebar_rename.update_title(&id, &new_title);
    });

    sidebar.setup_context_menu(session_id);
    SessionManager::wire_child_exited(manager, session_id);
    SessionManager::wire_focus_tracking(manager, session_id);
}

fn register_tab_actions(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
) {
    // tab-rename
    let sidebar_clone = sidebar.clone();
    let action = gio::SimpleAction::new("tab-rename", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };

        let sidebar_update = sidebar_clone.clone();
        let id_clone = id.clone();
        sidebar_clone.trigger_rename(&id, move |new_title| {
            sidebar_update.update_title(&id_clone, &new_title);
        });
    });
    window.add_action(&action);

    // tab-close
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("tab-close", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };
        mgr.borrow_mut().destroy_session(&id);
    });
    window.add_action(&action);

    // tab-close-others
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("tab-close-others", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };
        mgr.borrow_mut().close_others(&id);
    });
    window.add_action(&action);

    // group-delete
    let sidebar_del = sidebar.clone();
    let window_del = window.clone();
    let action = gio::SimpleAction::new("group-delete", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(group_id) = param.and_then(|v| v.get::<String>()) else { return };
        let tab_count = sidebar_del.tab_count_in_group(&group_id);

        if tab_count == 0 {
            sidebar_del.remove_group(&group_id);
        } else {
            let sidebar = sidebar_del.clone();
            let gid = group_id.clone();
            let dialog = gtk4::AlertDialog::builder()
                .message("Delete Group")
                .detail(&format!("This group has {tab_count} tab(s). Tabs will move to the default group."))
                .buttons(["Cancel", "Delete"])
                .default_button(1)
                .cancel_button(0)
                .build();

            dialog.choose(Some(&window_del), gio::Cancellable::NONE, move |result| {
                if result == Ok(1) {
                    sidebar.remove_group(&gid);
                }
            });
        }
    });
    window.add_action(&action);
}

fn register_terminal_actions(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
) {
    // term-copy
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("term-copy", None);
    action.connect_activate(move |_, _| {
        if let Some(term) = mgr.borrow().active_terminal_vte() {
            term.copy_clipboard_format(vte4::Format::Text);
        }
    });
    window.add_action(&action);

    // term-paste
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("term-paste", None);
    action.connect_activate(move |_, _| {
        if let Some(term) = mgr.borrow().active_terminal_vte() {
            term.paste_clipboard();
        }
    });
    window.add_action(&action);

    // split-h
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("split-h", None);
    action.connect_activate(move |_, _| {
        SessionManager::split_active_pane(&mgr, gtk4::Orientation::Horizontal);
    });
    window.add_action(&action);

    // split-v
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("split-v", None);
    action.connect_activate(move |_, _| {
        SessionManager::split_active_pane(&mgr, gtk4::Orientation::Vertical);
    });
    window.add_action(&action);

    // term-close (close pane or tab)
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("term-close", None);
    action.connect_activate(move |_, _| {
        let should_destroy = mgr.borrow_mut().close_active_pane();

        if should_destroy {
            let active = mgr.borrow().active_id().map(|s| s.to_string());

            if let Some(id) = active {
                mgr.borrow_mut().destroy_session(&id);
            }
        }
    });
    window.add_action(&action);
}

fn setup_terminal_context_menu(stack: &Stack) {
    let menu = gio::Menu::new();
    menu.append(Some("Copy"), Some("win.term-copy"));
    menu.append(Some("Paste"), Some("win.term-paste"));

    let split_section = gio::Menu::new();
    split_section.append(Some("Split Horizontal"), Some("win.split-h"));
    split_section.append(Some("Split Vertical"), Some("win.split-v"));
    menu.append_section(None, &split_section);

    let close_section = gio::Menu::new();
    close_section.append(Some("Close"), Some("win.term-close"));
    menu.append_section(None, &close_section);

    let popover = PopoverMenu::from_model(Some(&menu));
    popover.set_parent(stack);
    popover.set_has_arrow(false);

    let gesture = GestureClick::new();
    gesture.set_button(3);
    gesture.connect_released(move |gesture, _n_press, x, y| {
        gesture.set_state(gtk4::EventSequenceState::Claimed);
        popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.popup();
    });

    stack.add_controller(gesture);
}

fn show_new_group_dialog<F: Fn(String) + 'static>(window: &ApplicationWindow, on_create: F) {
    use gtk4::{Box as GtkBox, Button, Entry, Label, Orientation, Window};

    let dialog = Window::builder()
        .transient_for(window)
        .modal(true)
        .title("New Group")
        .default_width(300)
        .default_height(120)
        .resizable(false)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let label = Label::new(Some("Group name:"));
    label.set_xalign(0.0);

    let entry = Entry::new();
    entry.set_placeholder_text(Some("Enter group name"));

    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let create_btn = Button::with_label("Create");
    create_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&create_btn);

    vbox.append(&label);
    vbox.append(&entry);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let dialog_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_cancel.close();
    });

    let on_create = Rc::new(on_create);

    let dialog_create = dialog.clone();
    let entry_create = entry.clone();
    let on_create_btn = on_create.clone();
    create_btn.connect_clicked(move |_| {
        let name = entry_create.text().to_string();

        if !name.is_empty() {
            on_create_btn(name);
        }

        dialog_create.close();
    });

    let dialog_enter = dialog.clone();
    let entry_enter = entry.clone();
    entry.connect_activate(move |_| {
        let name = entry_enter.text().to_string();

        if !name.is_empty() {
            on_create(name);
        }

        dialog_enter.close();
    });

    dialog.present();
    entry.grab_focus();
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

fn setup_keyboard_shortcuts(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    on_new_tab: Rc<dyn Fn()>,
    extra_handler: Option<Rc<dyn Fn(Key, bool, bool) -> Option<glib::Propagation>>>,
) {
    let key_controller = EventControllerKey::new();
    key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

    let mgr = manager.clone();
    let notif_for_keys = notification_store.clone();

    key_controller.connect_key_pressed(move |_, key, _keycode, modifiers| {
        let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gtk4::gdk::ModifierType::ALT_MASK);

        let is_our_shortcut = (ctrl && shift && matches!(key, Key::C | Key::V | Key::T | Key::W | Key::N | Key::H | Key::E | Key::G | Key::Page_Up | Key::Page_Down))
            || (ctrl && !shift && matches!(key, Key::t | Key::Tab | Key::Page_Up | Key::Page_Down))
            || (alt && !ctrl && !shift && matches!(key, Key::h | Key::j | Key::k | Key::l))
            || (alt && matches!(key, Key::_1 | Key::_2 | Key::_3 | Key::_4 | Key::_5 | Key::_6 | Key::_7 | Key::_8 | Key::_9));

        if !is_our_shortcut {
            return glib::Propagation::Proceed;
        }

        // Let the extra handler try first (for window-specific shortcuts like new window/group)
        if let Some(ref handler) = extra_handler {
            if let Some(result) = handler(key, ctrl, shift) {
                return result;
            }
        }

        if ctrl && shift && key == Key::C {
            if let Some(term) = mgr.borrow().active_terminal_vte() {
                term.copy_clipboard_format(vte4::Format::Text);
            }
            return glib::Propagation::Stop;
        }

        if ctrl && shift && key == Key::V {
            if let Some(term) = mgr.borrow().active_terminal_vte() {
                term.paste_clipboard();
            }
            return glib::Propagation::Stop;
        }

        if ctrl && (key == Key::t || key == Key::T) {
            on_new_tab();
            return glib::Propagation::Stop;
        }

        if ctrl && shift && key == Key::H {
            SessionManager::split_active_pane(&mgr, gtk4::Orientation::Horizontal);
            return glib::Propagation::Stop;
        }

        if ctrl && shift && key == Key::E {
            SessionManager::split_active_pane(&mgr, gtk4::Orientation::Vertical);
            return glib::Propagation::Stop;
        }

        if alt && !ctrl && !shift {
            use crate::terminal::Direction;
            let direction = match key {
                Key::h => Some(Direction::Left),
                Key::l => Some(Direction::Right),
                Key::k => Some(Direction::Up),
                Key::j => Some(Direction::Down),
                _ => None,
            };

            if let Some(dir) = direction {
                mgr.borrow_mut().navigate_pane(dir);
                return glib::Propagation::Stop;
            }
        }

        if ctrl && shift && key == Key::W {
            let should_destroy = mgr.borrow_mut().close_active_pane();

            if should_destroy {
                let active = mgr.borrow().active_id().map(|s| s.to_string());

                if let Some(id) = active {
                    mgr.borrow_mut().destroy_session(&id);
                }
            }

            return glib::Propagation::Stop;
        }

        if ctrl && !shift && matches!(key, Key::Page_Down | Key::Page_Up) {
            if key == Key::Page_Down {
                mgr.borrow_mut().switch_next();
            } else {
                mgr.borrow_mut().switch_prev();
            }

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        if ctrl && shift && matches!(key, Key::Page_Down | Key::Page_Up) {
            if key == Key::Page_Down {
                mgr.borrow_mut().switch_next_group();
            } else {
                mgr.borrow_mut().switch_prev_group();
            }

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

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

                if let Some(active) = mgr.borrow().active_id() {
                    notif_for_keys.borrow_mut().mark_read(active);
                }

                return glib::Propagation::Stop;
            }
        }

        if ctrl && key == Key::Tab {
            if shift {
                mgr.borrow_mut().switch_prev();
            } else {
                mgr.borrow_mut().switch_next();
            }

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        glib::Propagation::Proceed
    });

    window.add_controller(key_controller);
}

fn setup_hook_polling(
    state: &Rc<AppState>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    dropdown: Option<Rc<crate::dropdown::DropdownWindow>>,
) {
    let hook_rx = state.take_hook_rx();
    let mgr_for_hooks = manager.clone();
    let notif_store = notification_store.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let Some(ref rx) = hook_rx else { return glib::ControlFlow::Continue };

        while let Ok(event) = rx.try_recv() {
            if event.event == "toggle-dropdown" {
                if let Some(dd) = dropdown.as_ref() {
                    dd.toggle();
                }
                continue;
            }

            let result = hook_handler::handle_hook_event(event);

            if let Some(status) = result.new_status {
                mgr_for_hooks.borrow_mut().update_session_status(&result.session_id, status);
            }

            if let Some(pid) = result.claude_pid {
                let pid_val = if pid == 0 { None } else { Some(pid) };
                mgr_for_hooks.borrow_mut().set_claude_pid(&result.session_id, pid_val);
            }

            if result.clear_notifications {
                notif_store.borrow_mut().clear_session(&result.session_id);
            }

            if let Some((title, subtitle, body)) = result.notification {
                let notification = crate::notifications::Notification::new(
                    &result.session_id,
                    &title,
                    &subtitle,
                    &body,
                );

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
}

fn setup_stale_pid_detection(manager: &Rc<RefCell<SessionManager>>) {
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
}

pub fn build_quake_window(app: &Application, state: &Rc<AppState>) {
    let dropdown = Rc::new(crate::dropdown::DropdownWindow::new(app, state));

    setup_hook_polling(state, &dropdown.manager, &dropdown.notification_store, Some(dropdown.clone()));
    setup_stale_pid_detection(&dropdown.manager);

    // Keyboard shortcuts — use dropdown's tab wiring (with close guard)
    let on_new_tab: Rc<dyn Fn()> = {
        let mgr = dropdown.manager.clone();
        let sid = dropdown.sidebar.clone();
        let notif = dropdown.notification_store.clone();

        Rc::new(move || {
            let id = mgr.borrow_mut().create_session(None, None);
            crate::dropdown::wire_tab(&sid, &mgr, &notif, &id);
        })
    };

    setup_keyboard_shortcuts(
        dropdown.window(),
        &dropdown.manager,
        &dropdown.notification_store,
        on_new_tab,
        None,
    );

    // Quit when compositor hides the window (WM close keybinding) —
    // layer-shell surfaces don't receive close_request, so we watch for
    // external visibility changes instead.
    let app_for_close = app.clone();
    let programmatic_hide = dropdown.programmatic_hide();
    dropdown.window().connect_notify_local(Some("visible"), move |window, _| {
        if !window.is_visible() && !programmatic_hide.get() {
            app_for_close.quit();
        }
    });

    // Present the window off-screen, ready for the first toggle
    dropdown.present_hidden();
}
