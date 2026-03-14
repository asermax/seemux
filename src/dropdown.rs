use std::cell::RefCell;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Revealer, RevealerTransitionType};

use crate::config::Config;

#[allow(dead_code)]
pub struct DropdownWindow {
    window: ApplicationWindow,
    revealer: Revealer,
    visible: RefCell<bool>,
}

#[allow(dead_code)]
impl DropdownWindow {
    pub fn new(app: &Application, config: &Config) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("seemux dropdown")
            .decorated(false)
            .default_width(1)
            .default_height(1)
            .build();

        // Get monitor dimensions
        let display = gtk4::gdk::Display::default().expect("display");
        let monitors = display.monitors();
        let (monitor_width, monitor_height) = if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
            let geom = monitor.geometry();
            (geom.width(), geom.height())
        } else {
            (1920, 1080)
        };

        let width = (monitor_width as f64 * config.dropdown_width_percent as f64 / 100.0) as i32;
        let height = (monitor_height as f64 * config.dropdown_height_percent as f64 / 100.0) as i32;

        window.set_default_size(width, height);

        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(config.dropdown_animation_ms);
        revealer.set_reveal_child(false);

        window.set_child(Some(&revealer));

        Self {
            window,
            revealer,
            visible: RefCell::new(false),
        }
    }

    pub fn set_content(&self, content: &gtk4::Widget) {
        self.revealer.set_child(Some(content));
    }

    pub fn toggle(&self) {
        let is_visible = *self.visible.borrow();

        if is_visible {
            self.revealer.set_reveal_child(false);

            // Hide window after animation completes
            let window = self.window.clone();
            let duration = self.revealer.transition_duration();
            gtk4::glib::timeout_add_local_once(
                std::time::Duration::from_millis(duration as u64),
                move || {
                    window.set_visible(false);
                },
            );

            *self.visible.borrow_mut() = false;
        } else {
            self.window.set_visible(true);
            self.window.present();
            self.revealer.set_reveal_child(true);
            *self.visible.borrow_mut() = true;
        }
    }

    pub fn window(&self) -> &ApplicationWindow {
        &self.window
    }
}
