pub mod manager;

use serde::{Deserialize, Serialize};

pub const DEFAULT_GROUP: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub status: SessionStatus,
    pub claude_pid: Option<u32>,
    pub claude_session_id: Option<String>,
    pub created_at: i64,
    pub cwd: Option<String>,
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    Idle,
    Running,
    NeedsInput,
    Completed,
    Error,
    Exited,
}

impl SessionStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Idle => "Idle",
            Self::Running => "Running",
            Self::NeedsInput => "Needs input",
            Self::Completed => "Completed",
            Self::Error => "Error",
            Self::Exited => "Exited",
        }
    }

    pub fn css_class(&self) -> &str {
        match self {
            Self::Idle => "status-pill--idle",
            Self::Running => "status-pill--running",
            Self::NeedsInput => "status-pill--needs-input",
            Self::Completed => "status-pill--completed",
            Self::Error => "status-pill--error",
            Self::Exited => "status-pill--idle",
        }
    }
}

impl Session {
    pub fn new(title: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            status: SessionStatus::Idle,
            claude_pid: None,
            claude_session_id: None,
            created_at: glib::DateTime::now_local()
                .map(|dt| dt.to_unix())
                .unwrap_or(0),
            cwd: None,
            group_id: DEFAULT_GROUP.to_string(),
        }
    }
}

use gtk4::glib;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_defaults() {
        let session = Session::new("Test Tab".to_string());

        assert_eq!(session.title, "Test Tab");
        assert_eq!(session.status, SessionStatus::Idle);
        assert_eq!(session.claude_pid, None);
        assert_eq!(session.claude_session_id, None);
        assert_eq!(session.cwd, None);
        assert_eq!(session.group_id, DEFAULT_GROUP);
        assert!(!session.id.is_empty());
        assert!(uuid::Uuid::parse_str(&session.id).is_ok());
    }

    #[test]
    fn session_status_labels() {
        assert_eq!(SessionStatus::Idle.label(), "Idle");
        assert_eq!(SessionStatus::Running.label(), "Running");
        assert_eq!(SessionStatus::NeedsInput.label(), "Needs input");
        assert_eq!(SessionStatus::Completed.label(), "Completed");
        assert_eq!(SessionStatus::Error.label(), "Error");
        assert_eq!(SessionStatus::Exited.label(), "Exited");
    }

    #[test]
    fn session_status_css_classes() {
        assert_eq!(SessionStatus::Idle.css_class(), "status-pill--idle");
        assert_eq!(SessionStatus::Running.css_class(), "status-pill--running");
        assert_eq!(SessionStatus::NeedsInput.css_class(), "status-pill--needs-input");
        assert_eq!(SessionStatus::Completed.css_class(), "status-pill--completed");
        assert_eq!(SessionStatus::Error.css_class(), "status-pill--error");
        assert_eq!(SessionStatus::Exited.css_class(), "status-pill--idle");
    }
}
