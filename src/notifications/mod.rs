pub mod hook_handler;
pub mod hook_server;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Notification {
    pub session_id: String,
    pub body: String,
}

impl Notification {
    pub fn new(session_id: &str, body: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            body: body.to_string(),
        }
    }
}

pub struct NotificationStore {
    unread_count_by_session: HashMap<String, u32>,
    latest_by_session: HashMap<String, Notification>,
    #[allow(clippy::type_complexity)]
    on_change: Option<Box<dyn Fn(&str, u32, Option<&Notification>, u32)>>,
}

impl NotificationStore {
    pub fn new() -> Self {
        Self {
            unread_count_by_session: HashMap::new(),
            latest_by_session: HashMap::new(),
            on_change: None,
        }
    }

    pub fn set_on_change<F: Fn(&str, u32, Option<&Notification>, u32) + 'static>(&mut self, f: F) {
        self.on_change = Some(Box::new(f));
    }

    pub fn add_notification(&mut self, notification: Notification) {
        let session_id = notification.session_id.clone();

        *self.unread_count_by_session.entry(session_id.clone()).or_insert(0) += 1;
        self.latest_by_session.insert(session_id.clone(), notification);

        self.notify_change(&session_id);
    }

    pub fn mark_read(&mut self, session_id: &str) {
        self.unread_count_by_session.insert(session_id.to_string(), 0);
        self.notify_change(session_id);
    }

    pub fn clear_session(&mut self, session_id: &str) {
        self.unread_count_by_session.remove(session_id);
        self.latest_by_session.remove(session_id);
        self.notify_change(session_id);
    }

    pub fn unread_count(&self, session_id: &str) -> u32 {
        *self.unread_count_by_session.get(session_id).unwrap_or(&0)
    }

    pub fn total_unread_count(&self) -> u32 {
        self.unread_count_by_session.values().sum()
    }

    fn notify_change(&self, session_id: &str) {
        if let Some(ref on_change) = self.on_change {
            let count = self.unread_count(session_id);
            let latest = self.latest_by_session.get(session_id);
            let total = self.total_unread_count();
            on_change(session_id, count, latest, total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_notif(session_id: &str, body: &str) -> Notification {
        Notification::new(session_id, body)
    }

    #[test]
    fn add_increments_unread_count() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "Permission needed"));
        store.add_notification(make_notif("s1", "Build failed"));

        assert_eq!(store.unread_count("s1"), 2);
        assert_eq!(store.unread_count("s2"), 0);
    }

    #[test]
    fn mark_read_resets_count() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "Permission needed"));
        store.add_notification(make_notif("s1", "Waiting for input"));
        store.mark_read("s1");

        assert_eq!(store.unread_count("s1"), 0);
    }

    #[test]
    fn clear_session_removes_all() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "Permission needed"));
        store.add_notification(make_notif("s2", "Build failed"));
        store.clear_session("s1");

        assert_eq!(store.unread_count("s1"), 0);
        assert_eq!(store.unread_count("s2"), 1);
        assert!(store.latest_by_session.get("s1").is_none());
        assert!(store.latest_by_session.get("s2").is_some());
    }

    #[test]
    fn latest_tracks_most_recent() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "first message"));
        store.add_notification(make_notif("s1", "second message"));

        let latest = store.latest_by_session.get("s1").unwrap();
        assert_eq!(latest.body, "second message");
    }

    #[test]
    fn total_unread_count_sums_all_sessions() {
        let mut store = NotificationStore::new();
        store.add_notification(make_notif("s1", "A"));
        store.add_notification(make_notif("s1", "B"));
        store.add_notification(make_notif("s2", "C"));

        assert_eq!(store.total_unread_count(), 3);

        store.mark_read("s1");
        assert_eq!(store.total_unread_count(), 1);

        store.clear_session("s2");
        assert_eq!(store.total_unread_count(), 0);
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
