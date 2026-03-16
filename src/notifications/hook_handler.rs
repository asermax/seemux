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
    pub notification: Option<(String, String, String)>, // (title, subtitle, body)
    pub clear_notifications: bool,
    pub claude_pid: Option<u32>,
}

pub fn handle_hook_event(event: HookEvent) -> HookResult {
    let mut result = HookResult {
        session_id: event.session_id.clone(),
        new_status: None,
        notification: None,
        clear_notifications: false,
        claude_pid: None,
    };

    match event.event.as_str() {
        "session-start" => {
            result.new_status = Some(SessionStatus::Running);
            result.clear_notifications = true;

            if let Some(pid) = event.payload.get("pid").and_then(|v| v.as_u64()) {
                result.claude_pid = Some(pid as u32);
            }
        }

        "prompt-submit" => {
            result.new_status = Some(SessionStatus::Running);
            result.clear_notifications = true;
        }

        "pre-tool-use" => {
            result.new_status = Some(SessionStatus::Running);
        }

        "notification" => {
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

            let (subtitle, body) = classify_notification(signal, message);
            result.notification = Some(("Claude Code".to_string(), subtitle, body));
            result.new_status = Some(SessionStatus::NeedsInput);
        }

        "stop" => {
            let message = event.payload.get("last_assistant_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Task completed");

            // Truncate to a reasonable preview length
            let preview: String = message.chars().take(100).collect();

            result.notification = Some((
                "Claude Code".to_string(),
                "Completed".to_string(),
                preview,
            ));
            result.new_status = Some(SessionStatus::Idle);
        }

        "session-end" => {
            result.new_status = Some(SessionStatus::Idle);
            result.claude_pid = Some(0); // signal to clear
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
        assert_eq!(body, "Claude reported an error");
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
        assert_eq!(body, "Claude needs your attention");
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
            "session-start",
            serde_json::json!({"pid": 12345}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::Running));
        assert!(result.clear_notifications);
        assert_eq!(result.claude_pid, Some(12345));
    }

    #[test]
    fn handle_prompt_submit() {
        let result = handle_hook_event(make_event("prompt-submit", serde_json::json!({})));

        assert_eq!(result.new_status, Some(SessionStatus::Running));
        assert!(result.clear_notifications);
    }

    #[test]
    fn handle_notification() {
        let result = handle_hook_event(make_event(
            "notification",
            serde_json::json!({"notification_type": "permission_prompt", "message": "Allow file write?"}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::NeedsInput));
        let (title, subtitle, body) = result.notification.unwrap();
        assert_eq!(title, "Claude Code");
        assert_eq!(subtitle, "Permission");
        assert_eq!(body, "Allow file write?");
    }

    #[test]
    fn handle_stop() {
        let result = handle_hook_event(make_event(
            "stop",
            serde_json::json!({"last_assistant_message": "Done refactoring"}),
        ));

        assert_eq!(result.new_status, Some(SessionStatus::Idle));
        let (_, subtitle, body) = result.notification.unwrap();
        assert_eq!(subtitle, "Completed");
        assert_eq!(body, "Done refactoring");
    }

    #[test]
    fn handle_session_end() {
        let result = handle_hook_event(make_event("session-end", serde_json::json!({})));

        assert_eq!(result.new_status, Some(SessionStatus::Idle));
        assert_eq!(result.claude_pid, Some(0));
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
            if message.is_empty() { "Claude reported an error".to_string() } else { message.to_string() },
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
        if message.is_empty() { "Claude needs your attention".to_string() } else { message.to_string() },
    )
}
