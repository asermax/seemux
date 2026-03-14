pub mod manager;

use serde::{Deserialize, Serialize};

pub const DEFAULT_GROUP: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub status: SessionStatus,
    pub claude_pid: Option<u32>,
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
            created_at: glib::DateTime::now_local()
                .map(|dt| dt.to_unix())
                .unwrap_or(0),
            cwd: None,
            group_id: DEFAULT_GROUP.to_string(),
        }
    }
}

use gtk4::glib;
