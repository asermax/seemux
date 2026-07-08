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
    check_window_visible: bool,
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
                    if event.event == "app.dropdown.toggle" {
                        if let Some(dd) = dropdown.as_ref() {
                            dd.toggle();
                        }
                        continue;
                    }

                    if event.event == "app.window.activate" {
                        window.present();
                        continue;
                    }

                    if event.event == "app.quit" {
                        if let Some(app) = window.application() {
                            app.quit();
                        }
                        continue;
                    }

                    if event.event == "agent.cwd.changed" {
                        if let Some(cwd) = event.payload.get("cwd").and_then(|v| v.as_str()) {
                            mgr_for_hooks.borrow_mut().update_session_cwd(&event.session_id, cwd);
                        }
                        continue;
                    }

                    // Skip notification events that arrive after a stop for the same turn.
                    // Agent hooks may fire both response.completed and attention.requested for completions;
                    // with async delivery the notification can arrive arbitrarily late.
                    if event.event == "agent.attention.requested"
                        && stopped_sessions.contains(&event.session_id)
                    {
                        continue;
                    }

                    if event.event == "agent.response.completed" || event.event == "agent.response.failed" {
                        stopped_sessions.insert(event.session_id.clone());
                    } else if matches!(
                        event.event.as_str(),
                        "agent.session.ended" | "agent.prompt.submitted" | "agent.tool.pre_use" | "agent.session.started"
                    ) {
                        stopped_sessions.remove(&event.session_id);
                    }

                    // Detect branch/PR changes: re-check after any git/gh Bash tool call
                    let should_redetect = (event.event == "agent.tool.post_use" || event.event == "agent.tool.failed")
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

                    if let Some(provider) = result.agent_provider {
                        mgr_for_hooks.borrow_mut().set_agent_provider(&result.session_id, Some(provider));
                    }

                    if let Some(pid) = result.agent_pid {
                        let pid_val = if pid == 0 { None } else { Some(pid) };
                        mgr_for_hooks.borrow_mut().set_agent_pid(&result.session_id, pid_val);

                        // session-end clears the agent binary name
                        if pid == 0 {
                            mgr_for_hooks.borrow_mut().set_agent_binary(&result.session_id, None);
                        }
                    }

                    if let Some(binary) = result.agent_binary {
                        mgr_for_hooks.borrow_mut().set_agent_binary(&result.session_id, Some(binary));
                    }

                    if let Some(agent_sid) = result.agent_session_id {
                        mgr_for_hooks.borrow_mut().set_agent_session_id(&result.session_id, agent_sid);
                    }

                    if result.clear_notifications {
                        notif_store.borrow_mut().clear_session(&result.session_id);
                    }

                    if let Some(body) = result.notification {
                        let is_active = mgr_for_hooks.borrow().active_id()
                            .map(|id| id == result.session_id)
                            .unwrap_or(false);

                        // In dropdown mode, also check if the window is actually visible —
                        // when hidden, the user can't see the terminal so badges should still appear.
                        let should_suppress = is_active && (!check_window_visible
                            || dropdown.as_ref().is_some_and(|dd| *dd.visible()));

                        if !should_suppress {
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
        let sessions = mgr_for_pid.borrow().sessions_with_agent_pid();

        for (session_id, pid, _provider) in sessions {
            let alive = unsafe { libc::kill(pid as i32, 0) } == 0;

            if !alive {
                let mut mgr = mgr_for_pid.borrow_mut();
                mgr.set_agent_pid(&session_id, None);
                mgr.set_agent_session_id(&session_id, None);
                mgr.set_agent_binary(&session_id, None);
                mgr.update_session_status(&session_id, SessionStatus::Idle);
            }
        }

        glib::ControlFlow::Continue
    });
}
