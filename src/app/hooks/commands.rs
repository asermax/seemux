use std::cell::RefCell;
use std::rc::Rc;

use crate::notifications::hook_server::CommandResponse;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

pub(super) fn handle_command(
    command: &str,
    params: &serde_json::Value,
    request_id: &str,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) -> CommandResponse {
    match command {
        "create-group" => cmd_create_group(request_id, params, manager, sidebar, notification_store),
        "create-session" => cmd_create_session(request_id, params, manager, sidebar, notification_store),
        "destroy-session" => cmd_destroy_session(request_id, params, manager),
        "focus-session" => cmd_focus_session(request_id, params, manager),
        "list-sessions" => cmd_list_sessions(request_id, params, sidebar),
        "send-input" => cmd_send_input(request_id, params, manager),
        _ => error_response(request_id, &format!("unknown command: {command}")),
    }
}

fn cmd_create_group(
    request_id: &str,
    params: &serde_json::Value,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) -> CommandResponse {
    let Some(name) = params.get("name").and_then(|v| v.as_str()) else {
        return error_response(request_id, "missing 'name' param");
    };

    let source_session_id = params.get("source_session_id").and_then(|v| v.as_str());

    // If the source session is already in a non-default group, use that group directly
    if let Some(sid) = source_session_id {
        let mgr = manager.borrow();

        if let Some(gid) = mgr.session_group_id(sid)
            && gid != crate::session::DEFAULT_GROUP {
                let gid = gid.to_string();
                return ok_response(request_id, serde_json::json!({ "group_id": gid }));
            }
    }

    // Find existing group by name or create a new one
    let group_id = sidebar.find_group_by_name(name)
        .unwrap_or_else(|| crate::app::create_group_programmatic(name, sidebar, manager, notification_store));

    if let Some(sid) = source_session_id {
        manager.borrow_mut().move_session_to_group(sid, &group_id);
    }

    ok_response(request_id, serde_json::json!({ "group_id": group_id }))
}

fn cmd_create_session(
    request_id: &str,
    params: &serde_json::Value,
    manager: &Rc<RefCell<SessionManager>>,
    sidebar: &Rc<Sidebar>,
    notification_store: &Rc<RefCell<NotificationStore>>,
) -> CommandResponse {
    let title = params.get("title").and_then(|v| v.as_str());
    let cwd = params.get("cwd").and_then(|v| v.as_str());
    let group_id = params.get("group_id").and_then(|v| v.as_str())
        .unwrap_or(crate::session::DEFAULT_GROUP);

    let argv: Option<Vec<String>> = params.get("argv")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let session_id = if let Some(argv) = &argv {
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();

        manager.borrow_mut().create_session_with_command_in_group(
            title.unwrap_or("Session"),
            cwd,
            group_id,
            &argv_refs,
        )
    } else {
        manager.borrow_mut().create_session_in_group(title, cwd, group_id)
    };

    crate::app::wire_tab_lifecycle(sidebar, manager, notification_store, &session_id);

    ok_response(request_id, serde_json::json!({ "session_id": session_id }))
}

fn cmd_destroy_session(
    request_id: &str,
    params: &serde_json::Value,
    manager: &Rc<RefCell<SessionManager>>,
) -> CommandResponse {
    let Some(session_id) = params.get("session_id").and_then(|v| v.as_str()) else {
        return error_response(request_id, "missing 'session_id' param");
    };

    manager.borrow_mut().destroy_session(session_id);

    ok_response(request_id, serde_json::json!({ "ok": true }))
}

fn cmd_focus_session(
    request_id: &str,
    params: &serde_json::Value,
    manager: &Rc<RefCell<SessionManager>>,
) -> CommandResponse {
    let Some(session_id) = params.get("session_id").and_then(|v| v.as_str()) else {
        return error_response(request_id, "missing 'session_id' param");
    };

    manager.borrow_mut().switch_to(session_id);

    ok_response(request_id, serde_json::json!({ "ok": true }))
}

fn cmd_list_sessions(
    request_id: &str,
    params: &serde_json::Value,
    sidebar: &Rc<Sidebar>,
) -> CommandResponse {
    let group_id = params.get("group_id").and_then(|v| v.as_str());

    let ids = if let Some(gid) = group_id {
        sidebar.ordered_session_ids_in_group(gid)
    } else {
        sidebar.ordered_session_ids()
    };

    let sessions: Vec<serde_json::Value> = ids.iter()
        .map(|id| serde_json::json!({ "session_id": id }))
        .collect();

    ok_response(request_id, serde_json::json!({ "sessions": sessions }))
}

fn cmd_send_input(
    request_id: &str,
    params: &serde_json::Value,
    manager: &Rc<RefCell<SessionManager>>,
) -> CommandResponse {
    let Some(session_id) = params.get("session_id").and_then(|v| v.as_str()) else {
        return error_response(request_id, "missing 'session_id' param");
    };

    let Some(text) = params.get("text").and_then(|v| v.as_str()) else {
        return error_response(request_id, "missing 'text' param");
    };

    let mgr = manager.borrow();

    if let Some(term) = mgr.session_terminal(session_id) {
        term.feed_child(text.as_bytes());
        ok_response(request_id, serde_json::json!({ "ok": true }))
    } else {
        error_response(request_id, &format!("session not found: {session_id}"))
    }
}

fn ok_response(request_id: &str, data: serde_json::Value) -> CommandResponse {
    CommandResponse {
        request_id: request_id.to_string(),
        status: "ok".to_string(),
        data,
    }
}

fn error_response(request_id: &str, message: &str) -> CommandResponse {
    CommandResponse {
        request_id: request_id.to_string(),
        status: "error".to_string(),
        data: serde_json::json!({ "error": message }),
    }
}
