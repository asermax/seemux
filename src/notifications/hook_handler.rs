use serde::Deserialize;

use crate::session::SessionStatus;

#[derive(Debug, Deserialize)]
pub struct HookEvent {
    pub event: String,
    pub session_id: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

pub struct HookResult {
    pub session_id: String,
    pub new_status: Option<SessionStatus>,
    pub notification: Option<String>,
    pub clear_notifications: bool,
    pub agent_pid: Option<u32>,
    pub agent_provider: Option<String>,
    pub agent_binary: Option<String>,
    /// Some(Some(id)) = set, Some(None) = clear, None = no change
    pub agent_session_id: Option<Option<String>>,
}

pub fn handle_hook_event(event: HookEvent) -> HookResult {
    let mut result = HookResult {
        session_id: event.session_id.clone(),
        new_status: None,
        notification: None,
        clear_notifications: false,
        agent_pid: None,
        agent_provider: None,
        agent_binary: None,
        agent_session_id: None,
    };

    match event.event.as_str() {
        "agent.session.started" => {
            result.new_status = Some(SessionStatus::Idle);
            result.clear_notifications = true;

            if let Some(pid) = event.payload.get("pid").and_then(|v| v.as_u64()) {
                result.agent_pid = Some(pid as u32);
            }

            result.agent_provider = event.payload.get("provider")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            result.agent_binary = event.payload.get("binary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            result.agent_session_id = event.payload.get("agent_session_id")
                .or_else(|| event.payload.get("session_id"))
                .and_then(|v| v.as_str())
                .map(|s| Some(s.to_string()));
        }

        "agent.prompt.submitted" => {
            result.new_status = Some(SessionStatus::Running);
            result.clear_notifications = true;
        }

        "agent.tool.pre_use" => {
            result.new_status = Some(SessionStatus::Running);
            result.clear_notifications = true;
        }

        "agent.attention.requested" => {
            let signal = event.payload.get("event_name")
                .or_else(|| event.payload.get("notification_type"))
                .or_else(|| event.payload.get("event"))
                .or_else(|| event.payload.get("reason"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let message = event.payload.get("message")
                .or_else(|| event.payload.get("body"))
                .or_else(|| event.payload.get("text"))
                .or_else(|| event.payload.get("prompt"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let (_, body) = classify_notification(signal, message);
            result.notification = Some(body);
            result.new_status = Some(SessionStatus::NeedsInput);
        }

        "agent.response.completed" => {
            let message = event.payload.get("last_message")
                .or_else(|| event.payload.get("last_assistant_message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Task completed");

            // Truncate to a reasonable preview length
            let preview: String = message.chars().take(100).collect();

            result.notification = Some(preview);
            result.new_status = Some(SessionStatus::Idle);
        }

        "agent.response.failed" => {
            let message = event.payload.get("message")
                .or_else(|| event.payload.get("error"))
                .or_else(|| event.payload.get("reason"))
                .and_then(|v| v.as_str())
                .unwrap_or("Turn ended due to an API error");

            let preview: String = message.chars().take(100).collect();

            result.notification = Some(preview);
            result.new_status = Some(SessionStatus::Error);
        }

        "agent.session.ended" => {
            result.new_status = Some(SessionStatus::Idle);
            result.agent_pid = Some(0); // signal to clear
            result.agent_session_id = Some(None); // signal to clear
        }

        _ => {}
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // classify_notification tests

    #[test]
    fn classify_permission_from_signal() {
        let (subtitle, _) = classify_notification("permission_prompt", "");
        assert_eq!(subtitle, "Permission");
    }

    #[test]
    fn classify_permission_from_message() {
        let (subtitle, body) = classify_notification("", "Approval needed for file write");
        assert_eq!(subtitle, "Permission");
        assert_eq!(body, "Approval needed for file write");
    }

    #[test]
    fn classify_error() {
        let (subtitle, body) = classify_notification("error", "Build failed");
        assert_eq!(subtitle, "Error");
        assert_eq!(body, "Build failed");
    }

    #[test]
    fn classify_error_default_body() {
        let (subtitle, body) = classify_notification("error", "");
        assert_eq!(subtitle, "Error");
        assert_eq!(body, "Agent reported an error");
    }

    #[test]
    fn classify_completed() {
        let (subtitle, _) = classify_notification("task_complete", "");
        assert_eq!(subtitle, "Completed");
    }

    #[test]
    fn classify_waiting() {
        let (subtitle, _) = classify_notification("idle_prompt", "");
        assert_eq!(subtitle, "Waiting");
    }

    #[test]
    fn classify_attention_fallback() {
        let (subtitle, body) = classify_notification("unknown", "Something happened");
        assert_eq!(subtitle, "Attention");
        assert_eq!(body, "Something happened");
    }

    #[test]
    fn classify_attention_empty() {
        let (subtitle, body) = classify_notification("", "");
        assert_eq!(subtitle, "Attention");
        assert_eq!(body, "Agent needs your attention");
    }

    #[test]
    fn classify_mixed_case() {
        let (subtitle, _) = classify_notification("Permission_PROMPT", "");
        assert_eq!(subtitle, "Permission");
    }

    #[test]
    fn classify_keyword_in_message_not_signal() {
        let (subtitle, _) = classify_notification("", "Task is done successfully");
        assert_eq!(subtitle, "Completed");
    }

    #[test]
    fn classify_multiple_keywords_first_wins() {
        // "permission" comes before "error" in the check order
        let (subtitle, _) = classify_notification("permission error", "");
        assert_eq!(subtitle, "Permission");
    }

    // handle_hook_event tests

    fn make_event(event: &str, payload: serde_json::Value) -> HookEvent {
        HookEvent {
            event: event.to_string(),
            session_id: "test-session".to_string(),
            payload,
        }
    }

    #[test]
    fn handle_session_start() {
        let result = handle_hook_event(make_event(
            "agent.session.started",
            serde_json::json!({"pid": 12345, "agent_session_id": "claude-abc-123"}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::Idle));
        assert!(result.clear_notifications);
        assert_eq!(result.agent_pid, Some(12345));
        assert_eq!(result.agent_session_id, Some(Some("claude-abc-123".to_string())));
    }

    #[test]
    fn handle_session_start_without_session_id() {
        let result = handle_hook_event(make_event(
            "agent.session.started",
            serde_json::json!({"pid": 12345}),
        ));

        assert_eq!(result.agent_session_id, None);
    }

    #[test]
    fn handle_session_start_reports_provider_without_pid() {
        // Claude Code's real SessionStart hook payload carries no `pid` field — only the
        // `pi` provider's extension supplies one via `process.pid`. Regression test for a bug
        // where the consumer only applied `agent_provider` inside the `pid.is_some()` branch,
        // silently discarding it for every real claude session and leaving a stale provider
        // (e.g. "pi") in place indefinitely.
        let result = handle_hook_event(make_event(
            "agent.session.started",
            serde_json::json!({"provider": "claude", "binary": "claude", "session_id": "claude-xyz"}),
        ));

        assert_eq!(result.agent_pid, None);
        assert_eq!(result.agent_provider, Some("claude".to_string()));
        assert_eq!(result.agent_session_id, Some(Some("claude-xyz".to_string())));
    }

    #[test]
    fn handle_prompt_submit() {
        let result = handle_hook_event(make_event("agent.prompt.submitted", serde_json::json!({})));

        assert_eq!(result.new_status, Some(SessionStatus::Running));
        assert!(result.clear_notifications);
    }

    #[test]
    fn handle_notification() {
        let result = handle_hook_event(make_event(
            "agent.attention.requested",
            serde_json::json!({"notification_type": "permission_prompt", "message": "Allow file write?"}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::NeedsInput));
        assert_eq!(result.notification.unwrap(), "Allow file write?");
    }

    #[test]
    fn handle_stop() {
        let result = handle_hook_event(make_event(
            "agent.response.completed",
            serde_json::json!({"last_message": "Done refactoring"}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::Idle));
        assert_eq!(result.notification.unwrap(), "Done refactoring");
    }

    #[test]
    fn handle_stop_failure() {
        let result = handle_hook_event(make_event(
            "agent.response.failed",
            serde_json::json!({"message": "Rate limit reached"}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::Error));
        assert_eq!(result.notification.unwrap(), "Rate limit reached");
    }

    #[test]
    fn handle_stop_failure_default_message() {
        let result = handle_hook_event(make_event("agent.response.failed", serde_json::json!({})));

        assert_eq!(result.new_status, Some(SessionStatus::Error));
        assert_eq!(result.notification.unwrap(), "Turn ended due to an API error");
    }

    #[test]
    fn handle_session_end() {
        let result = handle_hook_event(make_event("agent.session.ended", serde_json::json!({})));

        assert_eq!(result.new_status, Some(SessionStatus::Idle));
        assert_eq!(result.agent_pid, Some(0));
        assert_eq!(result.agent_session_id, Some(None));
    }

    #[test]
    fn handle_pre_tool_use() {
        let result = handle_hook_event(make_event(
            "agent.tool.pre_use",
            serde_json::json!({"tool_name": "Bash", "tool_input": {"command": "npm test"}}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::Running));
        assert!(result.clear_notifications);
    }

    #[test]
    fn handle_tool_failed() {
        let result = handle_hook_event(make_event(
            "agent.tool.failed",
            serde_json::json!({}),
        ));

        assert!(result.new_status.is_none());
        assert!(result.notification.is_none());
    }

    #[test]
    fn handle_cwd_changed() {
        let result = handle_hook_event(make_event(
            "agent.cwd.changed",
            serde_json::json!({"cwd": "/tmp"}),
        ));

        assert!(result.new_status.is_none());
        assert!(result.notification.is_none());
    }

    #[test]
    fn handle_unknown_event() {
        let result = handle_hook_event(make_event("bogus", serde_json::json!({})));

        assert!(result.new_status.is_none());
        assert!(result.notification.is_none());
    }
}

pub fn classify_notification(signal: &str, message: &str) -> (String, String) {
    let lower = format!("{signal} {message}").to_lowercase();

    if lower.contains("permission") || lower.contains("approve") || lower.contains("approval") {
        return (
            "Permission".to_string(),
            if message.is_empty() { "Approval needed".to_string() } else { message.to_string() },
        );
    }

    if lower.contains("error") || lower.contains("failed") || lower.contains("exception") {
        return (
            "Error".to_string(),
            if message.is_empty() { "Agent reported an error".to_string() } else { message.to_string() },
        );
    }

    if lower.contains("complet") || lower.contains("finish") || lower.contains("done") || lower.contains("success") {
        return (
            "Completed".to_string(),
            if message.is_empty() { "Task completed".to_string() } else { message.to_string() },
        );
    }

    if lower.contains("idle") || lower.contains("wait") || lower.contains("input") {
        return (
            "Waiting".to_string(),
            if message.is_empty() { "Waiting for input".to_string() } else { message.to_string() },
        );
    }

    (
        "Attention".to_string(),
        if message.is_empty() { "Agent needs your attention".to_string() } else { message.to_string() },
    )
}
