use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, GestureClick, Overlay, PopoverMenu, Stack, gdk, gio, glib};

use crate::notifications::NotificationStore;
use crate::persistence::StatePersistence;
use crate::session::manager::{self, SessionManager};
use crate::sidebar::Sidebar;
use crate::terminal::VteTerminal;

/// Intentional `vte4::Terminal` reference — GTK widget picking requires the concrete type.
/// This is the only VTE leak outside the terminal module.
fn find_url_at(gesture: &GestureClick, x: f64, y: f64) -> Option<String> {
    let stack_widget = gesture.widget()?;
    let picked = stack_widget.pick(x, y, gtk4::PickFlags::DEFAULT)?;

    let term = picked.ancestor(vte4::Terminal::static_type())
        .and_downcast::<vte4::Terminal>()
        .or_else(|| picked.downcast::<vte4::Terminal>().ok())?;

    let point = stack_widget.compute_point(&term, &gtk4::graphene::Point::new(x as f32, y as f32))?;
    VteTerminal::check_url_at(&term, point.x() as f64, point.y() as f64)
}

fn is_text_file_from_uri(url: &str) -> bool {
    manager::path_from_file_uri(url)
        .map(|p| is_text_file(std::path::Path::new(&p)))
        .unwrap_or(false)
}

pub(crate) fn register_tab_actions(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    persistence: &Rc<StatePersistence>,
) {
    // tab-close
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("tab-close", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };
        mgr.borrow_mut().destroy_session(&id);
        super::refocus_terminal(&mgr);
    });
    window.add_action(&action);

    // tab-close-others
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("tab-close-others", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(id) = param.and_then(|v| v.get::<String>()) else { return };
        mgr.borrow_mut().close_others(&id);
        super::refocus_terminal(&mgr);
    });
    window.add_action(&action);

    // group-delete
    let sidebar_del = sidebar.clone();
    let mgr_del = manager.clone();
    let persistence_del = persistence.clone();
    let action = gio::SimpleAction::new("group-delete", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(group_id) = param.and_then(|v| v.get::<String>()) else { return };
        let tab_count = sidebar_del.tab_count_in_group(&group_id);

        if tab_count == 0 {
            sidebar_del.remove_group(&group_id);
            persistence_del.mark_dirty();
            super::refocus_terminal(&mgr_del);
        } else {
            // Find the overlay by walking up from the sidebar container
            let Some(overlay) = sidebar_del.container.ancestor(Overlay::static_type())
                .and_downcast::<Overlay>() else { return };

            let sidebar = sidebar_del.clone();
            let gid = group_id.clone();
            let p = persistence_del.clone();

            super::dialogs::show_confirm_overlay(
                &overlay,
                &mgr_del,
                "Delete Group",
                &format!("This group has {tab_count} tab(s). Tabs will move to the default group."),
                move || {
                    sidebar.remove_group(&gid);
                    p.mark_dirty();
                },
            );
        }
    });
    window.add_action(&action);

    // group-rename
    let sidebar_rename = sidebar.clone();
    let mgr_rename = manager.clone();
    let persistence_rename = persistence.clone();
    let action = gio::SimpleAction::new("group-rename", Some(&String::static_variant_type()));
    action.connect_activate(move |_, param| {
        let Some(group_id) = param.and_then(|v| v.get::<String>()) else { return };
        let Some(current_name) = sidebar_rename.find_group_name(&group_id) else { return };

        let Some(overlay) = sidebar_rename.container.ancestor(Overlay::static_type())
            .and_downcast::<Overlay>() else { return };

        let sidebar = sidebar_rename.clone();
        let gid = group_id.clone();
        let p = persistence_rename.clone();

        super::dialogs::show_rename_group_overlay(
            &overlay,
            &mgr_rename,
            &current_name,
            move |new_name| {
                sidebar.rename_group(&gid, &new_name);
                p.mark_dirty();
            },
        );
    });
    window.add_action(&action);
}

pub(crate) fn register_terminal_actions(
    window: &ApplicationWindow,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) {
    // term-copy
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("term-copy", None);
    action.connect_activate(move |_, _| {
        if let Some(vt) = mgr.borrow().active_terminal_vte() {
            vt.copy_clipboard();
        }
    });
    window.add_action(&action);

    // term-paste
    let mgr = manager.clone();
    let action = gio::SimpleAction::new("term-paste", None);
    action.connect_activate(move |_, _| {
        if let Some(vt) = mgr.borrow().active_terminal_vte() {
            vt.paste_clipboard();
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

    // copy-url
    let action = gio::SimpleAction::new("copy-url", Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        let Some(url) = param.and_then(|v| v.get::<String>()) else { return };
        let Some(display) = gdk::Display::default() else { return };

        display.clipboard().set_text(&url);
    });
    window.add_action(&action);

    // open-url
    let action = gio::SimpleAction::new("open-url", Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        let Some(url) = param.and_then(|v| v.get::<String>()) else { return };

        if let Err(e) = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>) {
            eprintln!("Failed to open URL: {e}");
        }
    });
    window.add_action(&action);

    // edit-file — open a file:// URI in neovim, reusing the same instance per parent terminal
    let mgr = manager.clone();
    let sid = sidebar.clone();
    let notif = notification_store.clone();
    let editor_sessions: Rc<RefCell<std::collections::HashMap<String, String>>> =
        Rc::new(RefCell::new(std::collections::HashMap::new()));

    let action = gio::SimpleAction::new("edit-file", Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        let Some(url) = param.and_then(|v| v.get::<String>()) else { return };
        let Some(path) = manager::path_from_file_uri(&url) else { return };

        let filepath = std::path::Path::new(&path);

        if !filepath.is_file() {
            return;
        }

        let Some(parent_id) = mgr.borrow().active_id().map(|s| s.to_string()) else { return };

        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| "/tmp".to_string());
        let socket = format!("{runtime_dir}/seemux/nvim-{parent_id}.sock");

        // Try to reuse an existing editor for this parent terminal
        let reused = {
            let map = editor_sessions.borrow();

            if let Some(editor_id) = map.get(&parent_id) {
                let exists = mgr.borrow().session_terminal(editor_id).is_some();

                if exists && std::path::Path::new(&socket).exists() {
                    let _ = std::process::Command::new("nvim")
                        .args(["--server", &socket, "--remote", &path])
                        .spawn();

                    // Update subtitle to new file's parent directory
                    if let Some(parent_dir) = filepath.parent() {
                        let dir_path = parent_dir.to_string_lossy();
                        sid.update_subtitle(editor_id, &manager::display_path(&dir_path));
                    }

                    mgr.borrow_mut().switch_to(editor_id);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if reused {
            return;
        }

        // Clean up stale mapping and socket file
        editor_sessions.borrow_mut().remove(&parent_id);
        let _ = std::fs::remove_file(&socket);

        // Get parent terminal's CWD, fall back to file's parent dir
        let parent_cwd = mgr.borrow().session_terminal(&parent_id)
            .and_then(|vt| vt.current_directory_uri())
            .and_then(|uri| manager::path_from_file_uri(&uri))
            .or_else(|| filepath.parent().map(|p| p.to_string_lossy().to_string()));

        let filename = filepath.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());

        let id = mgr.borrow_mut().create_session_with_command(
            &filename,
            parent_cwd.as_deref(),
            &["nvim", "--listen", &socket, &path],
        );

        // Set folder subtitle (file's parent directory)
        if let Some(parent_dir) = filepath.parent() {
            let dir_path = parent_dir.to_string_lossy();
            sid.update_subtitle(&id, &manager::display_path(&dir_path));
        }

        editor_sessions.borrow_mut().insert(parent_id, id.clone());
        super::wire_tab_lifecycle(&sid, &mgr, &notif, &id);
    });
    window.add_action(&action);

    // open-in-browser — create browser session with given URL
    let mgr = manager.clone();
    let sid = sidebar.clone();
    let notif = notification_store.clone();
    let action = gio::SimpleAction::new("open-in-browser", Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        let Some(url) = param.and_then(|v| v.get::<String>()) else { return };
        let Some(url) = manager::normalize_url(&url) else { return };

        match mgr.borrow_mut().create_browser_session(&url) {
            Ok(id) => super::wire_tab_lifecycle(&sid, &mgr, &notif, &id),
            Err(msg) => show_browser_error_from_sidebar(&sid, &mgr, &msg),
        }
    });
    window.add_action(&action);

    // open-in-browser-split — add browser pane to current session
    let mgr = manager.clone();
    let sid = sidebar.clone();
    let action = gio::SimpleAction::new("open-in-browser-split", Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        let Some(url) = param.and_then(|v| v.get::<String>()) else { return };
        let Some(url) = manager::normalize_url(&url) else { return };

        if let Err(msg) = SessionManager::split_with_browser(&mgr, &url) {
            show_browser_error_from_sidebar(&sid, &mgr, &msg);
        }
    });
    window.add_action(&action);
}

fn show_browser_error_from_sidebar(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    message: &str,
) {
    let Some(overlay) = sidebar.container.ancestor(Overlay::static_type())
        .and_downcast::<Overlay>() else { return };
    super::dialogs::show_browser_error_overlay(&overlay, manager, message);
}

fn is_text_file(path: &std::path::Path) -> bool {
    let filename = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let (content_type, _uncertain) = gio::content_type_guess(Some(&filename), None::<&[u8]>);
    gio::content_type_is_a(&content_type, "text/plain")
}

pub(crate) fn setup_terminal_context_menu(stack: &Stack) {
    let popover = PopoverMenu::from_model(None::<&gio::MenuModel>);
    popover.set_parent(stack);
    popover.set_has_arrow(false);

    let gesture = GestureClick::new();
    gesture.set_button(3);

    gesture.connect_released(move |gesture, _n_press, x, y| {
        gesture.set_state(gtk4::EventSequenceState::Claimed);

        let menu = gio::Menu::new();

        let url = find_url_at(gesture, x, y);

        if let Some(ref url) = url {
            let url_section = gio::Menu::new();

            if url.starts_with("file://") {
                if is_text_file_from_uri(url) {
                    let item = gio::MenuItem::new(Some("Open in Editor"), None);
                    item.set_action_and_target_value(
                        Some("win.edit-file"),
                        Some(&url.to_variant()),
                    );
                    url_section.append_item(&item);
                }

                let item = gio::MenuItem::new(Some("Open with external App"), None);
                item.set_action_and_target_value(
                    Some("win.open-url"),
                    Some(&url.to_variant()),
                );
                url_section.append_item(&item);
            } else {
                let item = gio::MenuItem::new(Some("Open URL"), None);
                item.set_action_and_target_value(
                    Some("win.open-url"),
                    Some(&url.to_variant()),
                );
                url_section.append_item(&item);

                let browser_tab = gio::MenuItem::new(Some("Open in browser tab"), None);
                browser_tab.set_action_and_target_value(
                    Some("win.open-in-browser"),
                    Some(&url.to_variant()),
                );
                url_section.append_item(&browser_tab);

                let browser_split = gio::MenuItem::new(Some("Open in browser split"), None);
                browser_split.set_action_and_target_value(
                    Some("win.open-in-browser-split"),
                    Some(&url.to_variant()),
                );
                url_section.append_item(&browser_split);
            }

            menu.append_section(None, &url_section);
        }

        if let Some(ref url) = url {
            let copy_item = gio::MenuItem::new(Some("Copy URL"), None);
            copy_item.set_action_and_target_value(
                Some("win.copy-url"),
                Some(&url.to_variant()),
            );
            menu.append_item(&copy_item);
        } else {
            menu.append(Some("Copy"), Some("win.term-copy"));
        }
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

pub(crate) fn setup_ctrl_click_url_open(stack: &Stack) {
    let gesture = GestureClick::new();
    gesture.set_button(1);
    gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);

    gesture.connect_pressed(move |gesture, _n_press, x, y| {
        if !gesture.current_event_state().contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
            gesture.set_state(gtk4::EventSequenceState::Denied);
            return;
        }

        let Some(url) = find_url_at(gesture, x, y) else {
            gesture.set_state(gtk4::EventSequenceState::Denied);
            return;
        };

        gesture.set_state(gtk4::EventSequenceState::Claimed);
        let Some(stack_widget) = gesture.widget() else { return };

        if url.starts_with("file://") && is_text_file_from_uri(&url) {
            let _ = stack_widget.activate_action("win.edit-file", Some(&url.to_variant()));
        } else {
            let _ = stack_widget.activate_action("win.open-url", Some(&url.to_variant()));
        }
    });

    stack.add_controller(gesture);
}
