use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use vte4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, EventControllerKey, GestureClick, Orientation,
    Overlay, Paned, PopoverMenu, Stack, StackTransitionType,
    gio,
    gdk::Key,
    glib,
};

use crate::app_state::AppState;
use crate::config::SessionState;
use crate::notifications::hook_handler;
use crate::notifications::NotificationStore;
use crate::session::SessionStatus;
use crate::session::manager::{self, SessionManager};
use crate::sidebar::Sidebar;
use crate::terminal::VteTerminal;

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

    let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

    let manager = SessionManager::new(
        stack.clone(), sidebar.clone(), socket_path, config.clone(), notification_store.clone(),
    );

    // Wrap content in overlay for centered dialogs
    let overlay = Overlay::new();
    overlay.set_child(Some(&paned));

    // Common setup: actions, context menus, notification wiring, DnD
    setup_common(&window, &manager, &sidebar, &notification_store, &stack);

    // Quit when all tabs are closed
    let app_clone = app.clone();
    manager.borrow_mut().set_on_empty(move || {
        app_clone.quit();
    });

    // Restore saved sessions/groups or create a fresh tab
    restore_sessions(&sidebar, &manager, &notification_store);

    // Wire default group's "+ Add tab" button
    let mgr_new_tab = manager.clone();
    let sidebar_new_tab = sidebar.clone();
    let notif_new_tab = notification_store.clone();
    sidebar.connect_new_tab(move || {
        let id = mgr_new_tab.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(&sidebar_new_tab, &mgr_new_tab, &notif_new_tab, &id);
    });

    // Shared "create new group" logic — used by both sidebar button and Ctrl+Shift+G
    let create_group = make_create_group_action(
        &manager, &sidebar, &notification_store, &overlay,
        wire_tab_lifecycle,
    );

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

    setup_keyboard_shortcuts(&window, &manager, &sidebar, &notification_store, on_new_tab, extra_handler);

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

    window.set_child(Some(&overlay));
    window.present();

    // Spawn deferred shells, focus the first terminal, and resume Claude sessions
    let mgr_for_map = manager.clone();
    glib::idle_add_local_once(move || {
        let pending = mgr_for_map.borrow().sessions_pending_resume();
        mgr_for_map.borrow().spawn_deferred();

        if let Some(term) = mgr_for_map.borrow().active_terminal_vte() {
            term.grab_focus();
        }

        if !pending.is_empty() {
            let mgr = mgr_for_map.clone();

            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                for (session_id, claude_session_id) in &pending {
                    if let Some(term) = mgr.borrow().session_terminal(session_id) {
                        term.feed_child(format!("claude --resume {claude_session_id}\n").as_bytes());
                    }
                }
            });
        }
    });
}

/// Common setup shared by both normal and quake windows: context menu actions,
/// terminal right-click, notification badge wiring, and DnD tab reordering.
fn setup_common(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    stack: &Stack,
) {
    register_tab_actions(window, manager, sidebar);
    register_terminal_actions(window, manager);
    setup_terminal_context_menu(stack, manager);

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

    // Wire drag-and-drop tab movement/reordering
    let mgr_for_dnd = manager.clone();
    sidebar.set_on_tab_moved(move |session_id, new_group, position| {
        mgr_for_dnd.borrow_mut().move_session_to_position(&session_id, &new_group, position);
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
        refocus_terminal(&mgr);
    });

    sidebar.setup_context_menu(session_id);
    SessionManager::wire_child_exited(manager, session_id);
    SessionManager::wire_focus_tracking(manager, session_id);
    SessionManager::wire_bell(manager, session_id);
}

/// Restore saved groups and sessions from disk, or create a fresh tab if none exist.
fn restore_sessions(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) {
    let saved_state = SessionState::load();

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

fn register_tab_actions(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
) {
    // tab-close
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("tab-close", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };
        mgr.borrow_mut().destroy_session(&id);
        refocus_terminal(&mgr);
    });
    window.add_action(&action);

    // tab-close-others
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("tab-close-others", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };
        mgr.borrow_mut().close_others(&id);
        refocus_terminal(&mgr);
    });
    window.add_action(&action);

    // group-delete
    let sidebar_del = sidebar.clone();
    let mgr_del = manager.clone();
    let action = gio::SimpleAction::new("group-delete", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(group_id) = param.and_then(|v| v.get::<String>()) else { return };
        let tab_count = sidebar_del.tab_count_in_group(&group_id);

        if tab_count == 0 {
            sidebar_del.remove_group(&group_id);
            refocus_terminal(&mgr_del);
        } else {
            // Find the overlay by walking up from the sidebar container
            let Some(overlay) = sidebar_del.container.ancestor(Overlay::static_type())
                .and_downcast::<Overlay>() else { return };

            let sidebar = sidebar_del.clone();
            let gid = group_id.clone();

            show_confirm_overlay(
                &overlay,
                &mgr_del,
                "Delete Group",
                &format!("This group has {tab_count} tab(s). Tabs will move to the default group."),
                move || { sidebar.remove_group(&gid); },
            );
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

    // open-url
    let action = gio::SimpleAction::new("open-url", Some(glib::VariantTy::STRING));
    action.connect_activate(|_, param| {
        let Some(url) = param.and_then(|v| v.get::<String>()) else { return };

        if let Err(e) = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>) {
            eprintln!("Failed to open URL: {e}");
        }
    });
    window.add_action(&action);
}

fn setup_terminal_context_menu(stack: &Stack, manager: &Rc<RefCell<SessionManager>>) {
    let mgr = manager.clone();

    let popover = PopoverMenu::from_model(None::<&gio::MenuModel>);
    popover.set_parent(stack);
    popover.set_has_arrow(false);

    let gesture = GestureClick::new();
    gesture.set_button(3);

    gesture.connect_released(move |gesture, _n_press, x, y| {
        gesture.set_state(gtk4::EventSequenceState::Claimed);

        let menu = gio::Menu::new();

        // Check for URL at click position
        let url = mgr.borrow().active_terminal_vte().and_then(|term| {
            let stack_widget = gesture.widget()?;
            let point = stack_widget.compute_point(&term, &gtk4::graphene::Point::new(x as f32, y as f32))?;
            VteTerminal::check_url_at(&term, point.x() as f64, point.y() as f64)
        });

        if let Some(ref url) = url {
            let url_section = gio::Menu::new();
            let item = gio::MenuItem::new(Some("Open URL"), None);
            item.set_action_and_target_value(
                Some("win.open-url"),
                Some(&url.to_variant()),
            );
            url_section.append_item(&item);
            menu.append_section(None, &url_section);
        }

        menu.append(Some("Copy"), Some("win.term-copy"));
        menu.append(Some("Paste"), Some("win.term-paste"));

        let split_section = gio::Menu::new();
        split_section.append(Some("Split Horizontal"), Some("win.split-h"));
        split_section.append(Some("Split Vertical"), Some("win.split-v"));
        menu.append_section(None, &split_section);

        let close_section = gio::Menu::new();
        close_section.append(Some("Close"), Some("win.term-close"));
        menu.append_section(None, &close_section);

        popover.set_menu_model(Some(&menu));
        popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.popup();
    });

    stack.add_controller(gesture);
}

/// Build a closure that shows the "new group" overlay and wires the new group's
/// tab lifecycle. Used by both normal and quake windows.
fn make_create_group_action(
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    overlay: &Overlay,
    wire_tab: fn(&Rc<Sidebar>, &Rc<RefCell<SessionManager>>, &Rc<RefCell<NotificationStore>>, &str),
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
        show_new_group_overlay(&overlay, &mgr_for_overlay, move |name| {
            let group_id = uuid::Uuid::new_v4().to_string();
            sid.add_group(&group_id, &name);

            let mgr2 = mgr.clone();
            let sid2 = sid.clone();
            let notif2 = notif.clone();
            let gid = group_id.clone();
            let sid_expand = sid.clone();
            let gid_expand = group_id.clone();
            sid.connect_group_new_tab(&group_id, move |_| {
                sid_expand.expand_group(&gid_expand);
                let id = mgr2.borrow_mut().create_session_in_group(None, None, &gid);
                wire_tab(&sid2, &mgr2, &notif2, &id);
            });

            let first_id = mgr.borrow_mut().create_session_in_group(None, None, &group_id);
            wire_tab(&sid, &mgr, &notif, &first_id);
        });
    })
}

fn refocus_terminal(manager: &Rc<RefCell<SessionManager>>) {
    if let Some(term) = manager.borrow().active_terminal_vte() {
        term.grab_focus();
    }
}

/// Show a centered "New Group" form as an overlay child.
fn show_new_group_overlay<F: Fn(String) + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    on_create: F,
) {
    use gtk4::{Box as GtkBox, Button, Entry, Label};

    let card = GtkBox::new(Orientation::Vertical, 12);
    card.add_css_class("overlay-card");
    card.set_halign(gtk4::Align::Center);
    card.set_valign(gtk4::Align::Center);
    card.set_margin_top(16);
    card.set_margin_bottom(16);
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_width_request(300);

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

    card.append(&label);
    card.append(&entry);
    card.append(&btn_box);

    overlay.add_overlay(&card);

    let on_create = Rc::new(on_create);

    let overlay_cancel = overlay.clone();
    let card_cancel = card.clone();
    let mgr_cancel = manager.clone();
    cancel_btn.connect_clicked(move |_| {
        overlay_cancel.remove_overlay(&card_cancel);
        refocus_terminal(&mgr_cancel);
    });

    let overlay_create = overlay.clone();
    let card_create = card.clone();
    let entry_create = entry.clone();
    let on_create_btn = on_create.clone();
    let mgr_create = manager.clone();
    create_btn.connect_clicked(move |_| {
        let name = entry_create.text().to_string();

        if !name.is_empty() {
            on_create_btn(name);
        }

        overlay_create.remove_overlay(&card_create);
        refocus_terminal(&mgr_create);
    });

    let overlay_enter = overlay.clone();
    let card_enter = card.clone();
    let mgr_enter = manager.clone();
    entry.connect_activate(move |entry| {
        let name = entry.text().to_string();

        if !name.is_empty() {
            on_create(name);
        }

        overlay_enter.remove_overlay(&card_enter);
        refocus_terminal(&mgr_enter);
    });

    // Handle Escape to dismiss
    let key_controller = EventControllerKey::new();
    let overlay_esc = overlay.clone();
    let card_esc = card.clone();
    let mgr_esc = manager.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == Key::Escape {
            overlay_esc.remove_overlay(&card_esc);
            refocus_terminal(&mgr_esc);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    entry.add_controller(key_controller);

    entry.grab_focus();
}

/// Show a centered confirmation dialog as an overlay child.
fn show_confirm_overlay<F: Fn() + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    title: &str,
    detail: &str,
    on_confirm: F,
) {
    use gtk4::{Box as GtkBox, Button, Label};

    let card = GtkBox::new(Orientation::Vertical, 12);
    card.add_css_class("overlay-card");
    card.set_halign(gtk4::Align::Center);
    card.set_valign(gtk4::Align::Center);
    card.set_margin_top(16);
    card.set_margin_bottom(16);
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_width_request(300);

    let title_label = Label::new(Some(title));
    title_label.add_css_class("title-3");

    let detail_label = Label::new(Some(detail));
    detail_label.set_wrap(true);
    detail_label.set_xalign(0.0);

    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let confirm_btn = Button::with_label("Delete");
    confirm_btn.add_css_class("destructive-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&confirm_btn);

    card.append(&title_label);
    card.append(&detail_label);
    card.append(&btn_box);

    overlay.add_overlay(&card);

    let overlay_cancel = overlay.clone();
    let card_cancel = card.clone();
    let mgr_cancel = manager.clone();
    cancel_btn.connect_clicked(move |_| {
        overlay_cancel.remove_overlay(&card_cancel);
        refocus_terminal(&mgr_cancel);
    });

    let overlay_confirm = overlay.clone();
    let card_confirm = card.clone();
    let mgr_confirm = manager.clone();
    confirm_btn.connect_clicked(move |_| {
        on_confirm();
        overlay_confirm.remove_overlay(&card_confirm);
        refocus_terminal(&mgr_confirm);
    });
}

fn setup_keyboard_shortcuts(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    on_new_tab: Rc<dyn Fn()>,
    extra_handler: Option<Rc<dyn Fn(Key, bool, bool) -> Option<glib::Propagation>>>,
) {
    let key_controller = EventControllerKey::new();
    key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

    let mgr = manager.clone();
    let notif_for_keys = notification_store.clone();
    let sidebar_for_keys = sidebar.clone();

    key_controller.connect_key_pressed(move |_, key, _keycode, modifiers| {
        let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gtk4::gdk::ModifierType::ALT_MASK);

        let number_keys = matches!(key, Key::_1 | Key::_2 | Key::_3 | Key::_4 | Key::_5 | Key::_6 | Key::_7 | Key::_8 | Key::_9);

        let is_our_shortcut = (ctrl && shift && matches!(key, Key::C | Key::V | Key::T | Key::W | Key::N | Key::H | Key::E | Key::G))
            || (ctrl && !shift && matches!(key, Key::t | Key::Tab | Key::Page_Up | Key::Page_Down))
            || (alt && !ctrl && !shift && matches!(key, Key::h | Key::j | Key::k | Key::l | Key::Page_Up | Key::Page_Down))
            || (alt && !ctrl && shift && matches!(key, Key::Page_Up | Key::Page_Down))
            || (alt && ctrl && !shift && matches!(key, Key::Page_Up | Key::Page_Down))
            || ((alt || ctrl) && number_keys);

        if matches!(key, Key::Alt_L | Key::Alt_R) {
            sidebar_for_keys.show_tab_indices();
            return glib::Propagation::Proceed;
        }

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

        if !ctrl && !shift && alt && matches!(key, Key::Page_Down | Key::Page_Up)
            || ctrl && !shift && !alt && matches!(key, Key::Page_Down | Key::Page_Up)
        {
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

        if alt && !ctrl && shift && matches!(key, Key::Page_Down | Key::Page_Up) {
            // Try notification cycling first; fall back to regular tab cycling
            let had_notification = {
                let notifs = notif_for_keys.borrow();
                let mut mgr_mut = mgr.borrow_mut();

                if key == Key::Page_Down {
                    mgr_mut.switch_next_with_notifications(&notifs)
                } else {
                    mgr_mut.switch_prev_with_notifications(&notifs)
                }
            };

            if !had_notification {
                if key == Key::Page_Down {
                    mgr.borrow_mut().switch_next();
                } else {
                    mgr.borrow_mut().switch_prev();
                }
            }

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        if ctrl && alt && !shift && matches!(key, Key::Page_Down | Key::Page_Up) {
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

        if alt || (ctrl && number_keys) {
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

    let sidebar_for_release = sidebar.clone();
    key_controller.connect_key_released(move |_, key, _, _| {
        if matches!(key, Key::Alt_L | Key::Alt_R) {
            sidebar_for_release.hide_tab_indices();
        }
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

        // Track the last tool name per session for permission pill labels
        thread_local! {
            static LAST_TOOL: RefCell<std::collections::HashMap<String, String>> =
                RefCell::new(std::collections::HashMap::new());
        }

        while let Ok(event) = rx.try_recv() {
            if event.event == "toggle-dropdown" {
                if let Some(dd) = dropdown.as_ref() {
                    dd.toggle();
                }
                continue;
            }

            let result = hook_handler::handle_hook_event(event);

            // Track the last tool name for permission labels
            if let Some(ref tool) = result.tool_name {
                LAST_TOOL.with(|m| m.borrow_mut().insert(result.session_id.clone(), tool.clone()));
            }

            if let Some(status) = result.new_status {
                // Build a custom label for NeedsInput when we know the tool name
                let label_override = if status == SessionStatus::NeedsInput {
                    LAST_TOOL.with(|m| m.borrow().get(&result.session_id).map(|t| format!("Permission: {t}")))
                } else {
                    None
                };

                mgr_for_hooks.borrow_mut().update_session_status(
                    &result.session_id, status, label_override.as_deref(),
                );
            }

            if let Some(pid) = result.claude_pid {
                let pid_val = if pid == 0 { None } else { Some(pid) };
                mgr_for_hooks.borrow_mut().set_claude_pid(&result.session_id, pid_val);
            }

            if let Some(claude_sid) = result.claude_session_id {
                mgr_for_hooks.borrow_mut().set_claude_session_id(&result.session_id, claude_sid);
            }

            if result.clear_notifications {
                notif_store.borrow_mut().clear_session(&result.session_id);
            }

            if let Some((title, subtitle, body)) = result.notification {
                let is_active = mgr_for_hooks.borrow().active_id()
                    .map(|id| id == result.session_id)
                    .unwrap_or(false);

                if !is_active {
                    let notification = crate::notifications::Notification::new(
                        &result.session_id,
                        &title,
                        &subtitle,
                        &body,
                    );
                    notif_store.borrow_mut().add_notification(notification);
                }
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
                mgr_for_pid.borrow_mut().set_claude_session_id(&session_id, None);
                mgr_for_pid.borrow_mut().update_session_status(
                    &session_id,
                    crate::session::SessionStatus::Idle,
                    None,
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

    // Common setup: actions, context menus, notification wiring, DnD
    setup_common(
        dropdown.window(),
        &dropdown.manager,
        &dropdown.sidebar,
        &dropdown.notification_store,
        &dropdown.stack,
    );

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

    // Wire default group's "+ Add tab" button
    let mgr = dropdown.manager.clone();
    let sid = dropdown.sidebar.clone();
    let notif = dropdown.notification_store.clone();
    dropdown.sidebar.connect_new_tab(move || {
        let id = mgr.borrow_mut().create_session(None, None);
        wire_tab_lifecycle(&sid, &mgr, &notif, &id);
    });

    // Shared "create new group" logic — Ctrl+Shift+G and sidebar button
    let create_group = make_create_group_action(
        &dropdown.manager, &dropdown.sidebar, &dropdown.notification_store, &dropdown.overlay,
        wire_tab_lifecycle,
    );

    let create_group_btn = create_group.clone();
    dropdown.sidebar.connect_new_group(move || create_group_btn());

    // Keyboard shortcuts
    let on_new_tab: Rc<dyn Fn()> = {
        let mgr = dropdown.manager.clone();
        let sid = dropdown.sidebar.clone();
        let notif = dropdown.notification_store.clone();

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
            wire_tab_lifecycle(&sid, &mgr, &notif, &id);
        })
    };

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

    setup_keyboard_shortcuts(
        dropdown.window(),
        &dropdown.manager,
        &dropdown.sidebar,
        &dropdown.notification_store,
        on_new_tab,
        extra_handler,
    );

    // Track pointer position to distinguish external dialogs from
    // intentional focus switches. Layer-shell surfaces at LAYER_TOP receive
    // pointer enter/leave events regardless of keyboard focus state.
    let motion = gtk4::EventControllerMotion::new();

    let pointer_flag = dropdown.pointer_inside.clone();
    motion.connect_enter(move |_, _, _| {
        pointer_flag.set(true);
    });

    let pointer_flag = dropdown.pointer_inside.clone();
    motion.connect_leave(move |_| {
        pointer_flag.set(false);
    });

    dropdown.window().add_controller(motion);

    // Auto-hide when another window gets focus.
    // Use a short delay to avoid hiding when a popover (context menu)
    // briefly steals focus — the window becomes active again once the
    // popover closes.
    // If the pointer is inside the dropdown when focus is lost, an external
    // dialog stole focus — suspend (move off-screen) so the dialog becomes
    // accessible, then resume when focus returns.
    let hide_generation: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0));
    let dropdown_for_focus = dropdown.clone();
    let hide_gen = hide_generation.clone();
    let pointer_inside = dropdown.pointer_inside.clone();
    dropdown.window().connect_notify_local(Some("is-active"), move |window, _| {
        if !window.is_active() && *dropdown_for_focus.visible() {
            if pointer_inside.get() {
                // External dialog stole focus — move the dropdown off-screen
                // so the dialog (a normal xdg_toplevel) becomes accessible.
                dropdown_for_focus.suspend();
                return;
            }

            let current = hide_gen.get().wrapping_add(1);
            hide_gen.set(current);

            let dd = dropdown_for_focus.clone();
            let gen_check = hide_gen.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                if gen_check.get() == current && *dd.visible() {
                    dd.toggle();
                }
            });
        } else if window.is_active() {
            // Window became active again — resume if suspended, cancel pending hide.
            dropdown_for_focus.resume();
            hide_gen.set(hide_gen.get().wrapping_add(1));
        }
    });

    // Save session state on close
    let mgr_for_close = dropdown.manager.clone();
    let paned_for_close = dropdown.paned.clone();
    let config_for_close = state.config.clone();
    dropdown.window().connect_close_request(move |_| {
        mgr_for_close.borrow().save_state();

        let mut cfg = config_for_close.borrow_mut();
        cfg.sidebar_width = paned_for_close.position();
        cfg.save();

        glib::Propagation::Proceed
    });

    // Register global shortcut via XDG Portal (best-effort)
    let dropdown_for_shortcut = dropdown.clone();
    crate::global_shortcuts::register_toggle(move || dropdown_for_shortcut.toggle());

    // Spawn shell after layout and resume Claude sessions
    let mgr_spawn = dropdown.manager.clone();
    glib::idle_add_local_once(move || {
        let pending = mgr_spawn.borrow().sessions_pending_resume();
        mgr_spawn.borrow().spawn_deferred();

        if !pending.is_empty() {
            let mgr = mgr_spawn.clone();

            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                for (session_id, claude_session_id) in &pending {
                    if let Some(term) = mgr.borrow().session_terminal(session_id) {
                        term.feed_child(format!("claude --resume {claude_session_id}\n").as_bytes());
                    }
                }
            });
        }
    });

    // Present the window off-screen, ready for the first toggle
    dropdown.present_hidden();
}
