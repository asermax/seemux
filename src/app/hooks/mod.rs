use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::glib;

use crate::app_state::AppState;
use crate::notifications::hook_handler;
use crate::notifications::hook_server::SocketMessage;
use crate::notifications::NotificationStore;
use crate::session::SessionStatus;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

mod commands;

pub(crate) fn setup_hook_polling(
    state: &Rc<AppState>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    sidebar: &Rc<Sidebar>,
    dropdown: Option<Rc<crate::dropdown::DropdownWindow>>,
    window: gtk4::ApplicationWindow,
) {
    let hook_rx = state.take_hook_rx();
    let mgr_for_hooks = manager.clone();
    let notif_store = notification_store.clone();
    let manager_for_cmds = manager.clone();
    let sidebar_for_cmds = sidebar.clone();
    let notif_for_cmds = notification_store.clone();

    // Tracks sessions whose turn has completed, to suppress stale post-stop notifications.
    // Cleared when a new turn begins (prompt-submit, pre-tool-use, session-start) or session ends.
    let mut stopped_sessions: HashSet<String> = HashSet::new();

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

                    if event.event == "activate-window" {
                        window.present();
                        continue;
                    }

                    if event.event == "quit" {
                        if let Some(app) = window.application() {
                            app.quit();
                        }
                        continue;
                    }

                    // Skip notification events that arrive after a stop for the same turn.
                    // Claude Code fires both Stop and Notification hooks for completions;
                    // with async delivery the notification can arrive arbitrarily late.
                    if event.event == "notification"
                        && stopped_sessions.contains(&event.session_id)
                    {
                        continue;
                    }

                    if event.event == "stop" || event.event == "stop-failure" {
                        stopped_sessions.insert(event.session_id.clone());
                    } else if matches!(
                        event.event.as_str(),
                        "session-end" | "prompt-submit" | "pre-tool-use" | "session-start"
                    ) {
                        stopped_sessions.remove(&event.session_id);
                    }

                    // Detect branch/PR changes: re-check after any git/gh Bash tool call
                    let should_redetect = event.event == "post-tool-use"
                        && event.payload.get("tool_name").and_then(|v| v.as_str()) == Some("Bash")
                        && event.payload.get("tool_input")
                            .and_then(|ti| ti.get("command"))
                            .and_then(|v| v.as_str())
                            .is_some_and(crate::git::is_git_command);

                    let result = hook_handler::handle_hook_event(event);

                    if should_redetect {
                        mgr_for_hooks.borrow().redetect_branch_and_pr(&result.session_id);
                    }

                    if let Some(status) = result.new_status {
                        mgr_for_hooks.borrow_mut().update_session_status(
                            &result.session_id, status,
                        );
                    }

                    if let Some(pid) = result.claude_pid {
                        let pid_val = if pid == 0 { None } else { Some(pid) };
                        mgr_for_hooks.borrow_mut().set_claude_pid(&result.session_id, pid_val);

                        // session-end clears the Claude binary name
                        if pid == 0 {
                            mgr_for_hooks.borrow_mut().set_claude_binary(&result.session_id, None);
                        }
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
                mgr.set_claude_binary(&session_id, None);
                mgr.update_session_status(&session_id, SessionStatus::Idle);
            }
        }

        glib::ControlFlow::Continue
    });
}
