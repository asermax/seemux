use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, DrawingArea, GestureClick, Label, Orientation, ScrolledWindow, Separator,
};

use crate::session::SessionStatus;
use crate::theme::ColorScheme;

type Rgb = (f64, f64, f64);

pub const COLLAPSED_WIDTH: i32 = 36;

/// Precomputed RGB colors for zero-allocation draw calls.
struct DrawColors {
    idle: Rgb,
    running: Rgb,
    needs_input: Rgb,
    error: Rgb,
    completed: Rgb,
    accent: Rgb,
    sidebar_bg: Rgb,
}

impl DrawColors {
    fn from_scheme(s: &ColorScheme) -> Self {
        Self {
            idle: parse_hex(s.status_idle),
            running: parse_hex(s.status_running),
            needs_input: parse_hex(s.status_needs_input),
            error: parse_hex(s.status_error),
            completed: parse_hex(s.status_completed),
            accent: parse_hex(s.accent),
            sidebar_bg: parse_hex(s.sidebar_bg),
        }
    }

    fn status_rgb(&self, status: SessionStatus) -> Rgb {
        match status {
            SessionStatus::Idle | SessionStatus::Exited => self.idle,
            SessionStatus::Running => self.running,
            SessionStatus::NeedsInput => self.needs_input,
            SessionStatus::Error => self.error,
            SessionStatus::Completed => self.completed,
        }
    }
}

pub struct CollapsedBar {
    pub container: GtkBox,
    content: GtkBox,
    dots: Rc<RefCell<Vec<DotEntry>>>,
    separators: Rc<RefCell<HashMap<String, GtkBox>>>,
    #[allow(clippy::type_complexity)]
    on_dot_click: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    colors: Rc<DrawColors>,
}

struct DotEntry {
    session_id: String,
    group_id: String,
    drawing_area: DrawingArea,
    status: Rc<Cell<SessionStatus>>,
    active: Rc<Cell<bool>>,
    badge_count: Rc<Cell<u32>>,
}

fn parse_hex(hex: &str) -> Rgb {
    let hex = hex.trim_start_matches('#');

    let r = hex.get(0..2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(128) as f64 / 255.0;
    let g = hex.get(2..4).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(128) as f64 / 255.0;
    let b = hex.get(4..6).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(128) as f64 / 255.0;

    (r, g, b)
}

impl CollapsedBar {
    pub fn new(scheme: &'static ColorScheme) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("sidebar-collapsed");
        container.set_visible(false);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);

        let content = GtkBox::new(Orientation::Vertical, 2);
        content.set_halign(gtk4::Align::Center);

        scroll.set_child(Some(&content));
        container.append(&scroll);

        Self {
            container,
            content,
            dots: Rc::new(RefCell::new(Vec::new())),
            separators: Rc::new(RefCell::new(HashMap::new())),
            on_dot_click: Rc::new(RefCell::new(None)),
            colors: Rc::new(DrawColors::from_scheme(scheme)),
        }
    }

    pub fn set_on_dot_click<F: Fn(String) + 'static>(&self, f: F) {
        *self.on_dot_click.borrow_mut() = Some(Box::new(f));
    }

    /// Full rebuild from sidebar state.
    /// `sessions`: (session_id, group_id, status, is_active, badge_count)
    /// `groups`: (group_id, group_name)
    pub fn rebuild(
        &self,
        sessions: &[(String, String, SessionStatus, bool, u32)],
        groups: &[(String, String)],
    ) {
        // Clear existing widgets
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }

        self.dots.borrow_mut().clear();
        self.separators.borrow_mut().clear();

        // Build a group name lookup
        let group_names: HashMap<&str, &str> = groups.iter()
            .map(|(id, name)| (id.as_str(), name.as_str()))
            .collect();

        // Track which groups we've already inserted a separator for
        let mut seen_groups = std::collections::HashSet::new();
        seen_groups.insert("default".to_string());

        for (session_id, group_id, status, active, badge) in sessions {
            // Insert group separator if this is the first tab in a named group
            if group_id != "default" && seen_groups.insert(group_id.clone())
                && let Some(&name) = group_names.get(group_id.as_str())
            {
                let sep = build_group_separator(name);
                self.content.append(&sep);
                self.separators.borrow_mut().insert(group_id.clone(), sep);
            }

            let entry = self.build_dot(session_id, group_id, *status, *active, *badge);
            self.content.append(&entry.drawing_area);
            self.dots.borrow_mut().push(entry);
        }
    }

    pub fn update_status(&self, session_id: &str, status: &SessionStatus) {
        let dots = self.dots.borrow();

        if let Some(entry) = dots.iter().find(|d| d.session_id == session_id) {
            entry.status.set(*status);
            entry.drawing_area.queue_draw();
        }
    }

    pub fn update_active(&self, session_id: &str, active: bool) {
        let dots = self.dots.borrow();

        for entry in dots.iter() {
            if entry.session_id == session_id {
                entry.active.set(active);
                entry.drawing_area.queue_draw();
            } else if active && entry.active.get() {
                entry.active.set(false);
                entry.drawing_area.queue_draw();
            }
        }
    }

    pub fn update_badge(&self, session_id: &str, count: u32) {
        let dots = self.dots.borrow();

        if let Some(entry) = dots.iter().find(|d| d.session_id == session_id) {
            entry.badge_count.set(count);
            entry.drawing_area.queue_draw();
        }
    }

    pub fn add_dot(
        &self,
        session_id: &str,
        group_id: &str,
        status: SessionStatus,
        active: bool,
        badge: u32,
    ) {
        let entry = self.build_dot(session_id, group_id, status, active, badge);

        // Find the right position: after the last dot in the same group,
        // or after the group separator if no dots in the group yet
        let dots = self.dots.borrow();
        let last_in_group = dots.iter().rposition(|d| d.group_id == group_id);

        if let Some(idx) = last_in_group {
            let after_widget = &dots[idx].drawing_area;
            self.content.reorder_child_after(&entry.drawing_area, Some(after_widget.upcast_ref::<gtk4::Widget>()));
        } else if group_id != "default" {
            let seps = self.separators.borrow();

            if let Some(sep) = seps.get(group_id) {
                self.content.reorder_child_after(&entry.drawing_area, Some(sep.upcast_ref::<gtk4::Widget>()));
            }
        }

        drop(dots);
        self.dots.borrow_mut().push(entry);
    }

    pub fn remove_dot(&self, session_id: &str) {
        let mut dots = self.dots.borrow_mut();

        if let Some(idx) = dots.iter().position(|d| d.session_id == session_id) {
            let entry = dots.remove(idx);
            self.content.remove(&entry.drawing_area);
        }
    }

    pub fn add_group(&self, group_id: &str, name: &str) {
        let sep = build_group_separator(name);
        self.content.append(&sep);
        self.separators.borrow_mut().insert(group_id.to_string(), sep);
    }

    pub fn remove_group(&self, group_id: &str) {
        if let Some(sep) = self.separators.borrow_mut().remove(group_id) {
            self.content.remove(&sep);
        }
    }

    fn build_dot(
        &self,
        session_id: &str,
        group_id: &str,
        status: SessionStatus,
        active: bool,
        badge: u32,
    ) -> DotEntry {
        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(24);
        drawing_area.set_content_height(24);
        drawing_area.add_css_class("collapsed-dot");

        let status_cell = Rc::new(Cell::new(status));
        let active_cell = Rc::new(Cell::new(active));
        let badge_cell = Rc::new(Cell::new(badge));

        let colors = self.colors.clone();
        let status_draw = status_cell.clone();
        let active_draw = active_cell.clone();
        let badge_draw = badge_cell.clone();

        drawing_area.set_draw_func(move |_da, cr, width, height| {
            let cx = width as f64 / 2.0;
            let cy = height as f64 / 2.0;
            let radius = 5.0;

            // Status color (precomputed — no allocations)
            let (r, g, b) = colors.status_rgb(status_draw.get());

            cr.arc(cx, cy, radius, 0.0, 2.0 * std::f64::consts::PI);
            cr.set_source_rgb(r, g, b);
            let _ = cr.fill();

            // Active ring
            if active_draw.get() {
                let (ar, ag, ab) = colors.accent;
                cr.arc(cx, cy, radius + 2.5, 0.0, 2.0 * std::f64::consts::PI);
                cr.set_source_rgb(ar, ag, ab);
                cr.set_line_width(1.5);
                let _ = cr.stroke();
            }

            // Badge
            let count = badge_draw.get();

            if count > 0 {
                let badge_r = 5.0;
                let badge_cx = cx + radius + 1.0;
                let badge_cy = cy - radius - 1.0;

                let (ar, ag, ab) = colors.accent;
                cr.arc(badge_cx, badge_cy, badge_r, 0.0, 2.0 * std::f64::consts::PI);
                cr.set_source_rgb(ar, ag, ab);
                let _ = cr.fill();

                let (br, bg, bb) = colors.sidebar_bg;
                cr.set_source_rgb(br, bg, bb);
                cr.set_font_size(7.0);

                let text = if count > 9 { "9+".to_string() } else { count.to_string() };
                let extents = cr.text_extents(&text).unwrap();
                cr.move_to(
                    badge_cx - extents.width() / 2.0 - extents.x_bearing(),
                    badge_cy - extents.height() / 2.0 - extents.y_bearing(),
                );
                let _ = cr.show_text(&text);
            }
        });

        // Click handler
        let on_click = self.on_dot_click.clone();
        let sid = session_id.to_string();
        let gesture = GestureClick::new();
        gesture.set_button(1);

        gesture.connect_released(move |gesture, _n_press, _x, _y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);

            if let Some(ref callback) = *on_click.borrow() {
                callback(sid.clone());
            }
        });

        drawing_area.add_controller(gesture);

        DotEntry {
            session_id: session_id.to_string(),
            group_id: group_id.to_string(),
            drawing_area,
            status: status_cell,
            active: active_cell,
            badge_count: badge_cell,
        }
    }
}

fn build_group_separator(name: &str) -> GtkBox {
    let sep_box = GtkBox::new(Orientation::Horizontal, 2);
    sep_box.add_css_class("collapsed-group-sep");
    sep_box.set_margin_top(4);
    sep_box.set_margin_bottom(2);

    let line_left = Separator::new(Orientation::Horizontal);
    line_left.set_hexpand(true);
    line_left.add_css_class("collapsed-group-line");

    let initial = name.chars().next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    let label = Label::new(Some(&initial));
    label.add_css_class("collapsed-group-label");

    let line_right = Separator::new(Orientation::Horizontal);
    line_right.set_hexpand(true);
    line_right.add_css_class("collapsed-group-line");

    sep_box.append(&line_left);
    sep_box.append(&label);
    sep_box.append(&line_right);

    sep_box
}
