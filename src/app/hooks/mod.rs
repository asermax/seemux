use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::glib;

use crate::app_state::AppState;
use crate::notifications::hook_handler;
use crate::notifications::hook_server::SocketMessage;
use crate::notifications::NotificationStore;
use crate::session::SessionStatus;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

use std::time::{Duration, Instant};

mod commands;

pub(crate) fn setup_hook_polling(
    state: &Rc<AppState>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    sidebar: &Rc<Sidebar>,
    dropdown: Option<Rc<crate::dropdown::DropdownWindow>>,
) {
    let hook_rx = state.take_hook_rx();
    let mgr_for_hooks = manager.clone();
    let notif_store = notification_store.clone();
    let manager_for_cmds = manager.clone();
    let sidebar_for_cmds = sidebar.clone();
    let notif_for_cmds = notification_store.clone();

    // Tracks when each session last received a stop event, for post-stop notification suppression.
    let mut last_stop_by_session: HashMap<String, Instant> = HashMap::new();

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let Some(ref rx) = hook_rx else { return glib::ControlFlow::Continue };

        while let Ok(message) = rx.try_recv() {
            match message {
                SocketMessage::Hook(event) => {
                    if event.event == "toggle-dropdown" {
                        if let Some(dd) = dropdown.as_ref() {
                            dd.toggle();
                        }
                        continue;
                    }

                    // Skip notification events that arrive shortly after a stop,
                    // as Claude Code can fire both for the same completion.
                    if event.event == "notification" {
                        let recently_stopped = last_stop_by_session
                            .get(&event.session_id)
                            .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

                        if recently_stopped {
                            continue;
                        }
                    }

                    if event.event == "stop" || event.event == "stop-failure" {
                        last_stop_by_session.insert(event.session_id.clone(), Instant::now());
                    } else if event.event == "session-end" {
                        last_stop_by_session.remove(&event.session_id);
                    }

                    let result = hook_handler::handle_hook_event(event);

                    if let Some(status) = result.new_status {
                        mgr_for_hooks.borrow_mut().update_session_status(
                            &result.session_id, status,
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

                    if let Some(body) = result.notification {
                        let is_active = mgr_for_hooks.borrow().active_id()
                            .map(|id| id == result.session_id)
                            .unwrap_or(false);

                        if !is_active {
                            let notification = crate::notifications::Notification::new(
                                &result.session_id,
                                &body,
                            );
                            notif_store.borrow_mut().add_notification(notification);
                        }
                    }
                }

                SocketMessage::Command(request) => {
                    let response = commands::handle_command(
                        &request.command,
                        &request.params,
                        &request.request_id,
                        &manager_for_cmds,
                        &sidebar_for_cmds,
                        &notif_for_cmds,
                    );

                    let _ = request.response_tx.send(response);
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
                mgr.update_session_status(&session_id, SessionStatus::Idle);
            }
        }

        glib::ControlFlow::Continue
    });
}
