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
    #[allow(clippy::type_complexity)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_notif(session_id: &str, subtitle: &str) -> Notification {
        Notification {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            title: "Claude Code".to_string(),
            subtitle: subtitle.to_string(),
            body: "test body".to_string(),
            created_at: 0,
            is_read: false,
        }
    }

    #[test]
    fn add_increments_unread_count() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "Permission"));
        store.add_notification(make_notif("s1", "Error"));

        assert_eq!(store.unread_count("s1"), 2);
        assert_eq!(store.unread_count("s2"), 0);
    }

    #[test]
    fn mark_read_resets_count() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "Permission"));
        store.add_notification(make_notif("s1", "Waiting"));
        store.mark_read("s1");

        assert_eq!(store.unread_count("s1"), 0);
    }

    #[test]
    fn clear_session_removes_all() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "Permission"));
        store.add_notification(make_notif("s2", "Error"));
        store.clear_session("s1");

        assert_eq!(store.unread_count("s1"), 0);
        assert_eq!(store.unread_count("s2"), 1);
        assert_eq!(store.notifications.len(), 1);
    }

    #[test]
    fn latest_tracks_most_recent() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "First"));
        store.add_notification(make_notif("s1", "Second"));

        let latest = store.latest_by_session.get("s1").unwrap();
        assert_eq!(latest.subtitle, "Second");
    }

    #[test]
    fn independent_sessions() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "A"));
        store.add_notification(make_notif("s2", "B"));
        store.mark_read("s1");

        assert_eq!(store.unread_count("s1"), 0);
        assert_eq!(store.unread_count("s2"), 1);
    }
}
