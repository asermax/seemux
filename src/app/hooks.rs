use std::cell::RefCell;
use std::rc::Rc;

use gtk4::glib;

use crate::app_state::AppState;
use crate::notifications::hook_handler;
use crate::notifications::NotificationStore;
use crate::session::SessionStatus;
use crate::session::manager::SessionManager;

pub(crate) fn setup_hook_polling(
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
                LAST_TOOL.with(|m| m.borrow_mut().remove(&result.session_id));
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

pub(crate) fn setup_stale_pid_detection(manager: &Rc<RefCell<SessionManager>>) {
    let mgr_for_pid = manager.clone();

    glib::timeout_add_seconds_local(5, move || {
        let sessions = mgr_for_pid.borrow().sessions_with_claude_pid();

        for (session_id, pid) in sessions {
            let alive = unsafe { libc::kill(pid as i32, 0) } == 0;

            if !alive {
                let mut mgr = mgr_for_pid.borrow_mut();
                mgr.set_claude_pid(&session_id, None);
                mgr.set_claude_session_id(&session_id, None);
                mgr.update_session_status(
                    &session_id,
                    SessionStatus::Idle,
                    None,
                );
            }
        }

        glib::ControlFlow::Continue
    });
}
