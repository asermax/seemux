pub mod hook_handler;
pub mod hook_server;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub created_at: i64,
    pub is_read: bool,
}

impl Notification {
    pub fn new(session_id: &str, title: &str, subtitle: &str, body: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            body: body.to_string(),
            created_at: gtk4::glib::DateTime::now_local()
                .map(|dt| dt.to_unix())
                .unwrap_or(0),
            is_read: false,
        }
    }
}

pub struct NotificationStore {
    notifications: Vec<Notification>,
    unread_count_by_session: HashMap<String, u32>,
    latest_by_session: HashMap<String, Notification>,
    on_change: Option<Box<dyn Fn(&str, u32, Option<&Notification>)>>,
}

impl NotificationStore {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            unread_count_by_session: HashMap::new(),
            latest_by_session: HashMap::new(),
            on_change: None,
        }
    }

    pub fn set_on_change<F: Fn(&str, u32, Option<&Notification>) + 'static>(&mut self, f: F) {
        self.on_change = Some(Box::new(f));
    }

    pub fn add_notification(&mut self, notification: Notification) {
        let session_id = notification.session_id.clone();

        *self.unread_count_by_session.entry(session_id.clone()).or_insert(0) += 1;
        self.latest_by_session.insert(session_id.clone(), notification.clone());
        self.notifications.push(notification);

        self.notify_change(&session_id);
    }

    pub fn mark_read(&mut self, session_id: &str) {
        for n in &mut self.notifications {
            if n.session_id == session_id {
                n.is_read = true;
            }
        }

        self.unread_count_by_session.insert(session_id.to_string(), 0);
        self.notify_change(session_id);
    }

    pub fn clear_session(&mut self, session_id: &str) {
        self.notifications.retain(|n| n.session_id != session_id);
        self.unread_count_by_session.remove(session_id);
        self.latest_by_session.remove(session_id);
        self.notify_change(session_id);
    }

    pub fn unread_count(&self, session_id: &str) -> u32 {
        *self.unread_count_by_session.get(session_id).unwrap_or(&0)
    }

    fn notify_change(&self, session_id: &str) {
        if let Some(ref on_change) = self.on_change {
            let count = self.unread_count(session_id);
            let latest = self.latest_by_session.get(session_id);
            on_change(session_id, count, latest);
        }
    }
}
