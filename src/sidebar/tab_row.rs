use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::gio;
use gtk4::{Box as GtkBox, Button, GestureClick, Label, Orientation, PopoverMenu, Widget};
use gtk4::gdk;
use gtk4::glib;

use crate::session::SessionStatus;

pub struct TabRow {
    container: GtkBox,
    #[allow(dead_code)]
    active_indicator: GtkBox,
    #[allow(dead_code)]
    content: GtkBox,
    title_label: Label,
    subtitle_label: Label,
    branch_label: Label,
    pr_label: Label,
    status_label: Label,
    preview_label: Label,
    badge_label: Label,
    index_label: Label,
    close_btn: Button,
    peeking: Cell<bool>,
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
        title_label.set_tooltip_text(Some(title));

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

        let subtitle_label = Label::new(None);
        subtitle_label.add_css_class("tab-subtitle");
        subtitle_label.set_xalign(0.0);
        subtitle_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        subtitle_label.set_max_width_chars(30);
        subtitle_label.set_visible(false);

        let branch_label = Label::new(None);
        branch_label.add_css_class("tab-branch");
        branch_label.set_xalign(0.0);
        branch_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        branch_label.set_max_width_chars(20);
        branch_label.set_visible(false);

        let pr_label = Label::new(None);
        pr_label.add_css_class("tab-pr");
        pr_label.set_use_markup(true);
        pr_label.set_visible(false);

        let branch_row = GtkBox::new(Orientation::Horizontal, 4);
        branch_row.append(&branch_label);
        branch_row.append(&pr_label);

        content.append(&title_label);
        content.append(&subtitle_label);
        content.append(&branch_row);
        content.append(&status_label);
        content.append(&preview_label);

        // Badge (notification count)
        let badge_label = Label::new(None);
        badge_label.add_css_class("notification-badge");
        badge_label.set_visible(false);
        badge_label.set_valign(gtk4::Align::Center);

        // Tab index overlay (shown when Alt is held)
        let index_label = Label::new(None);
        index_label.add_css_class("tab-index");
        index_label.set_visible(false);
        index_label.set_valign(gtk4::Align::Center);

        // Close button
        let close_btn = Button::with_label("\u{00d7}");
        close_btn.add_css_class("tab-close-btn");
        close_btn.set_valign(gtk4::Align::Center);

        container.append(&active_indicator);
        container.append(&content);
        container.append(&badge_label);
        container.append(&index_label);
        container.append(&close_btn);

        Self {
            container,
            active_indicator,
            content,
            title_label,
            subtitle_label,
            branch_label,
            pr_label,
            status_label,
            preview_label,
            badge_label,
            index_label,
            close_btn,
            peeking: Cell::new(false),
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

    pub fn is_peeking(&self) -> bool {
        self.peeking.get()
    }

    pub fn set_peeking(&self, val: bool) {
        self.peeking.set(val);
    }

    pub fn is_active(&self) -> bool {
        self.container.has_css_class("active")
    }

    pub fn set_title(&self, title: &str) {
        self.title_label.set_text(title);
        self.title_label.set_tooltip_text(Some(title));
    }

    pub fn set_subtitle(&self, text: &str) {
        self.subtitle_label.set_text(text);
        self.subtitle_label.set_tooltip_text(Some(text));
        self.subtitle_label.set_visible(true);
    }

    pub fn update_cwd(&self, folder_name: &str, display_path: &str) {
        self.title_label.set_text(folder_name);
        self.title_label.set_tooltip_text(Some(display_path));

        self.subtitle_label.set_text(display_path);
        self.subtitle_label.set_tooltip_text(Some(display_path));
        self.subtitle_label.set_visible(true);
    }

    pub fn set_branch(&self, branch: Option<&str>) {
        match branch {
            Some(b) if !b.is_empty() => {
                self.branch_label.set_text(&format!("\u{e0a0} {b}")); // git branch icon
                self.branch_label.set_visible(true);
            }
            _ => {
                self.branch_label.set_visible(false);
                self.pr_label.set_visible(false);
            }
        }
    }

    pub fn set_pr(&self, pr: Option<(&str, &str)>) {
        match pr {
            Some((number, url)) => {
                let escaped_url = glib::markup_escape_text(url);
                self.pr_label.set_markup(
                    &format!("\u{2014} <a href=\"{escaped_url}\">PR#{number}</a>"),
                );
                self.pr_label.set_visible(true);
            }
            None => {
                self.pr_label.set_visible(false);
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

    pub fn has_badge(&self) -> bool {
        self.badge_label.is_visible()
    }

    pub fn set_index_visible(&self, index: Option<u32>) {
        match index {
            Some(i) if i <= 9 => {
                self.index_label.set_text(&i.to_string());
                self.index_label.set_visible(true);
                self.close_btn.set_visible(false);
            }
            _ => {
                self.index_label.set_visible(false);
                self.close_btn.set_visible(true);
            }
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

    pub fn setup_drag_source(&self, dragging_id: Rc<RefCell<String>>) {
        let drag_source = gtk4::DragSource::new();
        drag_source.set_actions(gdk::DragAction::MOVE);

        let container = self.container.clone();
        let dragging_prepare = dragging_id.clone();
        drag_source.connect_prepare(move |_source, _x, _y| {
            let session_id = container.widget_name().to_string();
            *dragging_prepare.borrow_mut() = session_id.clone();
            Some(gdk::ContentProvider::for_value(&session_id.to_value()))
        });

        let container = self.container.clone();
        drag_source.connect_drag_begin(move |source, _drag| {
            let paintable = gtk4::WidgetPaintable::new(Some(&container));
            source.set_icon(Some(&paintable), 0, 0);
            container.add_css_class("dragging");
        });

        let container = self.container.clone();
        let dragging_end = dragging_id.clone();
        drag_source.connect_drag_end(move |_source, _drag, _delete_data| {
            container.remove_css_class("dragging");
            dragging_end.borrow_mut().clear();
        });

        self.container.add_controller(drag_source);
    }

    /// Set up a drop target that shows a gap below this row and reports drops.
    /// The `on_drop` callback receives `(dragged_session_id, this_session_id)`.
    pub fn setup_drop_target<F: Fn(String, String) + 'static>(
        &self,
        dragging_id: Rc<RefCell<String>>,
        on_drop: F,
    ) {
        let drop_target = gtk4::DropTarget::new(glib::GString::static_type(), gdk::DragAction::MOVE);

        let container = self.container.clone();
        let dragging_motion = dragging_id.clone();
        drop_target.connect_motion(move |_target, _x, _y| {
            let dragged = dragging_motion.borrow();

            // Guard: reject group drags so they propagate to the group container's drop target
            if dragged.is_empty() {
                return gdk::DragAction::empty();
            }

            let target_id = container.widget_name();

            // Don't show indicator on self
            if dragged.as_str() == target_id.as_str() {
                return gdk::DragAction::MOVE;
            }

            // Don't show indicator on the row directly above the dragged item —
            // dropping after it would leave the dragged item in the same position.
            let is_above_dragged = container.parent()
                .and_then(|p| p.next_sibling())
                .and_then(|next| next.first_child())
                .is_some_and(|child| child.widget_name().as_str() == dragged.as_str());

            if is_above_dragged {
                return gdk::DragAction::MOVE;
            }

            container.add_css_class("drop-after");
            gdk::DragAction::MOVE
        });

        let container = self.container.clone();
        drop_target.connect_leave(move |_target| {
            container.remove_css_class("drop-after");
        });

        let container = self.container.clone();
        let on_drop = std::rc::Rc::new(on_drop);
        drop_target.connect_drop(move |_target, value, _x, _y| {
            container.remove_css_class("drop-after");

            let Ok(session_id) = value.get::<glib::GString>() else { return false };
            let target_id = container.widget_name().to_string();

            if session_id.as_str() == target_id {
                return false;
            }

            let is_above_dragged = container.parent()
                .and_then(|p| p.next_sibling())
                .and_then(|next| next.first_child())
                .is_some_and(|child| child.widget_name() == session_id.as_str());

            if is_above_dragged {
                return false;
            }

            on_drop(session_id.to_string(), target_id);
            true
        });

        self.container.add_controller(drop_target);
    }

    pub fn connect_close<F: Fn() + 'static>(&self, f: F) {
        self.close_btn.connect_clicked(move |_| f());
    }

    /// Set up right-click context menu with Close, Close Others.
    pub fn setup_context_menu(&self, session_id: &str) {
        let menu = gio::Menu::new();
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

}
