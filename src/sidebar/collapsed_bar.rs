use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, DrawingArea, GestureClick, Orientation, ScrolledWindow};

use crate::session::SessionStatus;
use crate::theme::ColorScheme;

type Rgb = (f64, f64, f64);

pub const COLLAPSED_WIDTH: i32 = 12;

/// Precomputed RGB colors for zero-allocation draw calls.
struct DrawColors {
    idle: Rgb,
    running: Rgb,
    needs_input: Rgb,
    error: Rgb,
    completed: Rgb,
    accent: Rgb,
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
    #[allow(clippy::type_complexity)]
    on_dot_click: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    colors: Rc<DrawColors>,
}

struct DotEntry {
    session_id: String,
    drawing_area: DrawingArea,
    status: Rc<Cell<SessionStatus>>,
    active: Rc<Cell<bool>>,
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

        let content = GtkBox::new(Orientation::Vertical, 1);
        content.set_halign(gtk4::Align::Center);

        scroll.set_child(Some(&content));
        container.append(&scroll);

        Self {
            container,
            content,
            dots: Rc::new(RefCell::new(Vec::new())),
            on_dot_click: Rc::new(RefCell::new(None)),
            colors: Rc::new(DrawColors::from_scheme(scheme)),
        }
    }

    pub fn set_on_dot_click<F: Fn(String) + 'static>(&self, f: F) {
        *self.on_dot_click.borrow_mut() = Some(Box::new(f));
    }

    /// Full rebuild from visible sessions.
    /// `sessions`: (session_id, status, is_active)
    pub fn rebuild(
        &self,
        sessions: &[(String, SessionStatus, bool)],
    ) {
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }

        self.dots.borrow_mut().clear();

        for (session_id, status, active) in sessions {
            let entry = self.build_dot(session_id, *status, *active);
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

    pub fn add_dot(
        &self,
        session_id: &str,
        status: SessionStatus,
        active: bool,
    ) {
        let entry = self.build_dot(session_id, status, active);
        self.content.append(&entry.drawing_area);
        self.dots.borrow_mut().push(entry);
    }

    pub fn remove_dot(&self, session_id: &str) {
        let mut dots = self.dots.borrow_mut();

        if let Some(idx) = dots.iter().position(|d| d.session_id == session_id) {
            let entry = dots.remove(idx);
            self.content.remove(&entry.drawing_area);
        }
    }

    fn build_dot(
        &self,
        session_id: &str,
        status: SessionStatus,
        active: bool,
    ) -> DotEntry {
        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(10);
        drawing_area.set_content_height(10);
        drawing_area.add_css_class("collapsed-dot");

        let status_cell = Rc::new(Cell::new(status));
        let active_cell = Rc::new(Cell::new(active));

        let colors = self.colors.clone();
        let status_draw = status_cell.clone();
        let active_draw = active_cell.clone();

        drawing_area.set_draw_func(move |_da, cr, width, height| {
            let cx = width as f64 / 2.0;
            let cy = height as f64 / 2.0;
            let radius = 4.0;

            // Status color
            let (r, g, b) = colors.status_rgb(status_draw.get());

            cr.arc(cx, cy, radius, 0.0, 2.0 * std::f64::consts::PI);
            cr.set_source_rgb(r, g, b);
            let _ = cr.fill();

            // Active ring
            if active_draw.get() {
                let (ar, ag, ab) = colors.accent;
                cr.arc(cx, cy, 5.0, 0.0, 2.0 * std::f64::consts::PI);
                cr.set_source_rgb(ar, ag, ab);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
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
            drawing_area,
            status: status_cell,
            active: active_cell,
        }
    }
}
