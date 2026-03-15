use gtk4::prelude::*;
use gtk4::gio;
use gtk4::{Box as GtkBox, Button, Entry, GestureClick, Label, Orientation, PopoverMenu, Widget};
use gtk4::gdk;

use crate::session::SessionStatus;

pub struct TabRow {
    container: GtkBox,
    #[allow(dead_code)]
    active_indicator: GtkBox,
    content: GtkBox,
    title_label: Label,
    branch_label: Label,
    status_label: Label,
    preview_label: Label,
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

        // Content area (title + status + preview stacked vertically)
        let content = GtkBox::new(Orientation::Vertical, 1);
        content.set_hexpand(true);

        let title_label = Label::new(Some(title));
        title_label.add_css_class("tab-title");
        title_label.set_xalign(0.0);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let status_label = Label::new(None);
        status_label.add_css_class("status-pill");
        status_label.set_xalign(0.0);
        status_label.set_visible(false);

        let preview_label = Label::new(None);
        preview_label.add_css_class("tab-notification-preview");
        preview_label.set_xalign(0.0);
        preview_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        preview_label.set_max_width_chars(25);
        preview_label.set_visible(false);

        let branch_label = Label::new(None);
        branch_label.add_css_class("tab-branch");
        branch_label.set_xalign(0.0);
        branch_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        branch_label.set_max_width_chars(20);
        branch_label.set_visible(false);

        content.append(&title_label);
        content.append(&branch_label);
        content.append(&status_label);
        content.append(&preview_label);

        // Badge (notification count)
        let badge_label = Label::new(None);
        badge_label.add_css_class("notification-badge");
        badge_label.set_visible(false);
        badge_label.set_valign(gtk4::Align::Center);

        // Close button
        let close_btn = Button::with_label("\u{00d7}");
        close_btn.add_css_class("tab-close-btn");
        close_btn.set_valign(gtk4::Align::Center);

        container.append(&active_indicator);
        container.append(&content);
        container.append(&badge_label);
        container.append(&close_btn);

        Self {
            container,
            active_indicator,
            content,
            title_label,
            branch_label,
            status_label,
            preview_label,
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

    pub fn set_branch(&self, branch: Option<&str>) {
        match branch {
            Some(b) if !b.is_empty() => {
                self.branch_label.set_text(&format!("\u{e0a0} {b}")); // git branch icon
                self.branch_label.set_visible(true);
            }
            _ => {
                self.branch_label.set_visible(false);
            }
        }
    }

    pub fn set_badge_count(&self, count: u32) {
        if count > 0 {
            self.badge_label.set_text(&count.to_string());
            self.badge_label.set_visible(true);
        } else {
            self.badge_label.set_visible(false);
        }
    }

    pub fn set_notification_preview(&self, text: Option<&str>) {
        match text {
            Some(t) if !t.is_empty() => {
                self.preview_label.set_text(t);
                self.preview_label.set_visible(true);
            }
            _ => {
                self.preview_label.set_visible(false);
            }
        }
    }

    pub fn set_status(&self, status: &SessionStatus) {
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

    pub fn setup_drag_source(&self) {
        let drag_source = gtk4::DragSource::new();
        drag_source.set_actions(gdk::DragAction::MOVE);

        let container = self.container.clone();
        drag_source.connect_prepare(move |_source, _x, _y| {
            let session_id = container.widget_name().to_string();
            Some(gdk::ContentProvider::for_value(&session_id.to_value()))
        });

        let container = self.container.clone();
        drag_source.connect_drag_begin(move |source, _drag| {
            let paintable = gtk4::WidgetPaintable::new(Some(&container));
            source.set_icon(Some(&paintable), 0, 0);
            container.add_css_class("dragging");
        });

        let container = self.container.clone();
        drag_source.connect_drag_end(move |_source, _drag, _delete_data| {
            container.remove_css_class("dragging");
        });

        self.container.add_controller(drag_source);
    }

    pub fn connect_close<F: Fn() + 'static>(&self, f: F) {
        self.close_btn.connect_clicked(move |_| f());
    }

    /// Set up right-click context menu with Rename, Close, Close Others.
    pub fn setup_context_menu(&self, session_id: &str) {
        let menu = gio::Menu::new();
        menu.append(Some("Rename"), Some(&format!("win.tab-rename('{session_id}')")));
        menu.append(Some("Close"), Some(&format!("win.tab-close('{session_id}')")));
        menu.append(Some("Close Others"), Some(&format!("win.tab-close-others('{session_id}')")));

        let popover = PopoverMenu::from_model(Some(&menu));
        popover.set_parent(&self.container);
        popover.set_has_arrow(false);

        let gesture = GestureClick::new();
        gesture.set_button(3);

        gesture.connect_released(move |gesture, _n_press, x, y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        });

        self.container.add_controller(gesture);
    }

    /// Start inline rename. Shows an Entry replacing the title label.
    pub fn start_rename<F: Fn(String) + Clone + 'static>(&self, on_rename: F) {
        let content = &self.content;
        let title_label = &self.title_label;

        let current_title = title_label.text().to_string();
        let original = current_title.clone();

        let entry = Entry::new();
        entry.set_text(&current_title);
        entry.set_hexpand(true);
        entry.add_css_class("tab-rename-entry");

        title_label.set_visible(false);
        content.prepend(&entry);
        entry.grab_focus();
        entry.select_region(0, -1);

        let content_enter = content.clone();
        let label_enter = title_label.clone();
        let on_rename_enter = on_rename.clone();
        let entry_focus = entry.clone();

        entry.connect_activate(move |entry| {
            let new_title = entry.text().to_string();
            content_enter.remove(entry);
            label_enter.set_text(if new_title.is_empty() { &current_title } else { &new_title });
            label_enter.set_visible(true);

            if !new_title.is_empty() && new_title != current_title {
                on_rename_enter(new_title);
            }
        });

        let content_focus = content.clone();
        let label_focus = title_label.clone();

        let focus_controller = gtk4::EventControllerFocus::new();
        focus_controller.connect_leave(move |_| {
            if entry_focus.parent().is_some() {
                content_focus.remove(&entry_focus);
                label_focus.set_text(&original);
                label_focus.set_visible(true);
            }
        });

        entry.add_controller(focus_controller);
    }

    /// Set up double-click to rename.
    pub fn connect_rename<F: Fn(String) + Clone + 'static>(&self, on_rename: F) {
        let content = self.content.clone();
        let title_label = self.title_label.clone();

        let gesture = GestureClick::new();
        gesture.set_button(1);

        gesture.connect_released(move |gesture, n_press, _, _| {
            if n_press != 2 {
                return;
            }

            gesture.set_state(gtk4::EventSequenceState::Claimed);

            // Reuse start_rename logic inline (can't call self.start_rename from closure)
            let current_title = title_label.text().to_string();
            let original = current_title.clone();

            let entry = Entry::new();
            entry.set_text(&current_title);
            entry.set_hexpand(true);
            entry.add_css_class("tab-rename-entry");

            title_label.set_visible(false);
            content.prepend(&entry);
            entry.grab_focus();
            entry.select_region(0, -1);

            let content_enter = content.clone();
            let label_enter = title_label.clone();
            let on_rename_enter = on_rename.clone();
            let entry_focus = entry.clone();

            entry.connect_activate(move |entry| {
                let new_title = entry.text().to_string();
                content_enter.remove(entry);
                label_enter.set_text(if new_title.is_empty() { &current_title } else { &new_title });
                label_enter.set_visible(true);

                if !new_title.is_empty() && new_title != current_title {
                    on_rename_enter(new_title);
                }
            });

            let content_focus = content.clone();
            let label_focus = title_label.clone();

            let focus_controller = gtk4::EventControllerFocus::new();
            focus_controller.connect_leave(move |_| {
                if entry_focus.parent().is_some() {
                    content_focus.remove(&entry_focus);
                    label_focus.set_text(&original);
                    label_focus.set_visible(true);
                }
            });

            entry.add_controller(focus_controller);
        });

        self.title_label.add_controller(gesture);
    }
}
