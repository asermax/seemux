use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, Widget};

use crate::session::SessionStatus;

pub struct TabRow {
    container: GtkBox,
    active_indicator: GtkBox,
    title_label: Label,
    status_label: Label,
    badge_label: Label,
    close_btn: Button,
}

impl TabRow {
    pub fn new(id: &str, title: &str) -> Self {
        let container = GtkBox::new(Orientation::Horizontal, 6);
        container.add_css_class("tab-row");
        container.set_widget_name(id);

        // Left rail active indicator
        let active_indicator = GtkBox::new(Orientation::Vertical, 0);
        active_indicator.add_css_class("active-indicator");
        active_indicator.set_valign(gtk4::Align::Fill);

        // Content area (title + status stacked vertically)
        let content = GtkBox::new(Orientation::Vertical, 2);
        content.set_hexpand(true);

        let title_label = Label::new(Some(title));
        title_label.add_css_class("tab-title");
        title_label.set_xalign(0.0);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let status_label = Label::new(None);
        status_label.add_css_class("status-pill");
        status_label.set_xalign(0.0);
        status_label.set_visible(false);

        content.append(&title_label);
        content.append(&status_label);

        // Badge (notification count)
        let badge_label = Label::new(None);
        badge_label.add_css_class("notification-badge");
        badge_label.set_visible(false);
        badge_label.set_valign(gtk4::Align::Center);

        // Close button
        let close_btn = Button::with_label("\u{00d7}"); // × character
        close_btn.add_css_class("tab-close-btn");
        close_btn.set_valign(gtk4::Align::Center);

        container.append(&active_indicator);
        container.append(&content);
        container.append(&badge_label);
        container.append(&close_btn);

        Self {
            container,
            active_indicator,
            title_label,
            status_label,
            badge_label,
            close_btn,
        }
    }

    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    pub fn set_active(&self, active: bool) {
        if active {
            self.container.add_css_class("active");
        } else {
            self.container.remove_css_class("active");
        }
    }

    pub fn set_title(&self, title: &str) {
        self.title_label.set_text(title);
    }

    pub fn set_badge_count(&self, count: u32) {
        if count > 0 {
            self.badge_label.set_text(&count.to_string());
            self.badge_label.set_visible(true);
        } else {
            self.badge_label.set_visible(false);
        }
    }

    pub fn set_status(&self, status: &SessionStatus) {
        // Remove all status CSS classes
        for class in &[
            "status-pill--idle",
            "status-pill--running",
            "status-pill--needs-input",
            "status-pill--completed",
            "status-pill--error",
        ] {
            self.status_label.remove_css_class(class);
        }

        match status {
            SessionStatus::Idle | SessionStatus::Exited => {
                self.status_label.set_visible(false);
            }
            _ => {
                self.status_label.set_text(status.label());
                self.status_label.add_css_class(status.css_class());
                self.status_label.set_visible(true);
            }
        }
    }

    pub fn connect_close<F: Fn() + 'static>(&self, f: F) {
        self.close_btn.connect_clicked(move |_| f());
    }
}
