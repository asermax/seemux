use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, EventControllerKey, gdk::Key, glib};

use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

#[allow(clippy::type_complexity)]
pub(crate) fn setup_keyboard_shortcuts(
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

    key_controller.connect_key_pressed(move |_, key, keycode, modifiers| {
        let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gtk4::gdk::ModifierType::ALT_MASK);

        // Translate keycode to the base (unshifted) keysym so Ctrl+Shift+[ works
        // regardless of what shifted character the layout produces
        let base_key = gtk4::gdk::Display::default()
            .and_then(|d| d.translate_key(keycode, gtk4::gdk::ModifierType::empty(), 0))
            .map(|(k, _, _, _)| k);
        let is_bracket = matches!(base_key, Some(Key::bracketleft) | Some(Key::bracketright));

        let number_keys = matches!(key, Key::_1 | Key::_2 | Key::_3 | Key::_4 | Key::_5 | Key::_6 | Key::_7 | Key::_8 | Key::_9);

        #[allow(clippy::nonminimal_bool)]
        let is_our_shortcut = (ctrl && shift && (matches!(key, Key::B | Key::C | Key::V | Key::T | Key::W | Key::N | Key::H | Key::E | Key::G | Key::Page_Up | Key::Page_Down) || is_bracket))
            || (ctrl && !shift && matches!(key, Key::Page_Up | Key::Page_Down))
            || (ctrl && key == Key::Tab)
            || (alt && !ctrl && !shift && matches!(key, Key::h | Key::j | Key::k | Key::l | Key::Page_Up | Key::Page_Down))
            || (alt && !ctrl && shift && matches!(key, Key::Page_Up | Key::Page_Down))
            || (alt && ctrl && !shift && matches!(key, Key::Page_Up | Key::Page_Down))
            || ((alt || ctrl) && number_keys);

        if matches!(key, Key::Alt_L | Key::Alt_R) && !ctrl && !shift {
            sidebar_for_keys.show_tab_indices();
            return glib::Propagation::Proceed;
        }

        if !is_our_shortcut {
            return glib::Propagation::Proceed;
        }

        // Let the extra handler try first (for window-specific shortcuts like new window/group)
        if let Some(ref handler) = extra_handler
            && let Some(result) = handler(key, ctrl, shift) {
                return result;
        }

        if ctrl && shift && key == Key::C {
            if let Some(vt) = mgr.borrow().active_terminal_vte() {
                vt.copy_clipboard();
            }
            return glib::Propagation::Stop;
        }

        if ctrl && shift && key == Key::V {
            if let Some(vt) = mgr.borrow().active_terminal_vte() {
                vt.paste_clipboard();
            }
            return glib::Propagation::Stop;
        }

        if ctrl && shift && key == Key::T {
            on_new_tab();
            return glib::Propagation::Stop;
        }

        if ctrl && shift && key == Key::B {
            sidebar_for_keys.set_sidebar_collapsed(!sidebar_for_keys.is_sidebar_collapsed());
            return glib::Propagation::Stop;
        }

        if ctrl && shift && is_bracket {
            if let Some(group_id) = mgr.borrow().active_group_id() {
                if base_key == Some(Key::bracketleft) {
                    sidebar_for_keys.collapse_group(group_id);
                } else {
                    sidebar_for_keys.expand_group(group_id);
                }
            }

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
            mgr.borrow_mut().switch_adjacent(key == Key::Page_Down);

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        if alt && !ctrl && shift && matches!(key, Key::Page_Down | Key::Page_Up) {
            // Try notification cycling first; fall back to regular tab cycling
            let forward = key == Key::Page_Down;

            let had_notification = {
                let notifs = notif_for_keys.borrow();
                mgr.borrow_mut().switch_adjacent_with_notifications(&notifs, forward)
            };

            if !had_notification {
                mgr.borrow_mut().switch_adjacent(forward);
            }

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        if ctrl && alt && !shift && matches!(key, Key::Page_Down | Key::Page_Up) {
            mgr.borrow_mut().switch_adjacent_group(key == Key::Page_Down);

            if let Some(active) = mgr.borrow().active_id() {
                notif_for_keys.borrow_mut().mark_read(active);
            }

            return glib::Propagation::Stop;
        }

        if ctrl && shift && !alt && matches!(key, Key::Page_Down | Key::Page_Up) {
            mgr.borrow_mut().switch_adjacent_running(key == Key::Page_Down);

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
                mgr.borrow_mut().switch_to_visible_index(idx);

                if let Some(active) = mgr.borrow().active_id() {
                    notif_for_keys.borrow_mut().mark_read(active);
                }

                return glib::Propagation::Stop;
            }
        }

        if ctrl && key == Key::Tab {
            mgr.borrow_mut().switch_adjacent(!shift);

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
