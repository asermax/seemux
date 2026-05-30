use std::cell::RefCell;
use std::rc::Rc;

use crate::notifications::hook_server::CommandResponse;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::{GroupPlacement, Sidebar};

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

/// Outcome of the team-group placement decision: reuse an existing group, or
/// create a new one at the resolved sidebar position.
#[derive(Debug, PartialEq)]
enum GroupResolution {
    Reuse(String),
    Create(GroupPlacement),
}

/// Decide where a team group lands relative to the lead session's current group.
/// Priority: existing same-name group (R4) → lead alone in a named group (R3) →
/// lead in a populated named group (R2) → lead in default / no lead (R1).
fn resolve_team_group(
    existing_named: Option<String>,
    lead_group: Option<String>,
    lead_group_is_default: bool,
    lead_group_tab_count: usize,
) -> GroupResolution {
    if let Some(gid) = existing_named {
        return GroupResolution::Reuse(gid);
    }

    match lead_group {
        Some(gid) if !lead_group_is_default => {
            if lead_group_tab_count <= 1 {
                GroupResolution::Reuse(gid)
            } else {
                GroupResolution::Create(GroupPlacement::After(gid))
            }
        }
        _ => GroupResolution::Create(GroupPlacement::First),
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

    let lead_group = source_session_id
        .and_then(|sid| manager.borrow().session_group_id(sid).map(|g| g.to_string()));

    let lead_group_is_default = lead_group.as_deref()
        .is_none_or(|g| g == crate::session::DEFAULT_GROUP);

    let lead_group_tab_count = lead_group.as_deref()
        .map(|g| sidebar.tab_count_in_group(g))
        .unwrap_or(0);

    let group_id = match resolve_team_group(
        sidebar.find_group_by_name(name),
        lead_group,
        lead_group_is_default,
        lead_group_tab_count,
    ) {
        GroupResolution::Reuse(gid) => gid,
        GroupResolution::Create(placement) => {
            crate::app::create_group_programmatic(name, placement, sidebar, manager, notification_store)
        }
    };

    // No-op when the lead already sits in the target group (R3, or R4 re-entry).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r4_reuses_existing_named_group_over_everything() {
        // Existing same-name group wins even when the lead is in a populated group.
        let resolution = resolve_team_group(
            Some("team-foo".to_string()),
            Some("backend".to_string()),
            false,
            5,
        );
        assert_eq!(resolution, GroupResolution::Reuse("team-foo".to_string()));
    }

    #[test]
    fn r3_reuses_group_when_lead_is_alone() {
        let resolution = resolve_team_group(None, Some("backend".to_string()), false, 1);
        assert_eq!(resolution, GroupResolution::Reuse("backend".to_string()));
    }

    #[test]
    fn r2_creates_after_populated_named_group() {
        let resolution = resolve_team_group(None, Some("backend".to_string()), false, 3);
        assert_eq!(
            resolution,
            GroupResolution::Create(GroupPlacement::After("backend".to_string())),
        );
    }

    #[test]
    fn r1_creates_first_when_lead_in_default_group() {
        let resolution = resolve_team_group(None, Some("default".to_string()), true, 4);
        assert_eq!(resolution, GroupResolution::Create(GroupPlacement::First));
    }

    #[test]
    fn r1_creates_first_when_no_lead() {
        let resolution = resolve_team_group(None, None, true, 0);
        assert_eq!(resolution, GroupResolution::Create(GroupPlacement::First));
    }
}
