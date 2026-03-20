use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Orientation, Overlay, Paned,
    Stack, StackTransitionType,
};
#[cfg(feature = "layer-shell")]
use gtk4::glib;

use crate::app_state::AppState;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

pub struct DropdownWindow {
    window: ApplicationWindow,
    visible: RefCell<bool>,

    #[cfg(feature = "layer-shell")]
    target_height: i32,
    #[cfg(feature = "layer-shell")]
    animation_ms: u32,
    #[cfg(feature = "layer-shell")]
    /// Incremented on each animation start; stale callbacks see a mismatch and stop.
    animation_generation: Rc<Cell<u32>>,

    #[cfg(feature = "kwin")]
    revealer: gtk4::Revealer,

    last_keypress: Rc<Cell<Option<Instant>>>,
    pub overlay: Overlay,
    pub paned: Paned,
    pub stack: Stack,
    pub manager: Rc<RefCell<SessionManager>>,
    pub notification_store: Rc<RefCell<NotificationStore>>,
    pub sidebar: Rc<Sidebar>,
}

impl DropdownWindow {
    pub fn new(app: &Application, state: &Rc<AppState>) -> Self {
        let config = state.config.clone();
        let cfg = config.borrow();

        let window = ApplicationWindow::builder()
            .application(app)
            .title("seemux dropdown")
            .decorated(false)
            .build();

        window.add_css_class("dropdown-window");

        // Get monitor dimensions
        let display = gtk4::gdk::Display::default().expect("display");
        let monitors = display.monitors();
        let (monitor_width, monitor_height) = if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
            let geom = monitor.geometry();
            (geom.width(), geom.height())
        } else {
            (1920, 1080)
        };

        let width = (monitor_width as f64 * cfg.dropdown_width_percent as f64 / 100.0) as i32;
        let target_height = (monitor_height as f64 * cfg.dropdown_height_percent as f64 / 100.0) as i32;

        #[cfg(feature = "layer-shell")]
        {
            crate::layer_shell::setup_dropdown(&window, width, monitor_width, -target_height);
        }

        #[cfg(feature = "kwin")]
        {
            let x = (monitor_width - width) / 2;
            crate::kwin_rules::install_rules(width, target_height, x, 0);
        }

        window.set_default_size(width, target_height);

        // Build full UI
        let scheme = crate::theme::get_scheme(&cfg.color_scheme);
        let sidebar = Rc::new(Sidebar::new(scheme));
        sidebar.wire_collapse_toggle();

        let stack = Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.set_transition_type(StackTransitionType::None);

        let paned = Paned::new(Orientation::Horizontal);
        paned.set_start_child(Some(&sidebar.container));
        paned.set_end_child(Some(&stack));
        paned.set_position(cfg.sidebar_width);
        paned.set_wide_handle(true);
        paned.set_shrink_start_child(false);
        paned.set_shrink_end_child(false);
        paned.set_resize_start_child(false);
        paned.set_resize_end_child(true);

        let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

        let manager = SessionManager::new(
            stack.clone(),
            sidebar.clone(),
            state.socket_path.clone(),
            config.clone(),
            notification_store.clone(),
        );

        // Border wrapper with overlay for centered dialogs
        let content = GtkBox::new(Orientation::Vertical, 0);
        content.add_css_class("dropdown-border");
        content.set_vexpand(true);
        content.append(&paned);

        let overlay = Overlay::new();
        overlay.set_child(Some(&content));

        #[cfg(feature = "layer-shell")]
        {
            window.set_child(Some(&overlay));
        }

        #[cfg(feature = "kwin")]
        let revealer = {
            let revealer = gtk4::Revealer::new();
            revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
            revealer.set_transition_duration(cfg.dropdown_animation_ms);
            revealer.set_reveal_child(false);
            revealer.set_child(Some(&overlay));
            window.set_child(Some(&revealer));

            // Hide window once the slide-up animation finishes
            let window_for_reveal = window.clone();
            revealer.connect_notify_local(Some("child-revealed"), move |rev, _| {
                if !rev.is_child_revealed() {
                    window_for_reveal.set_visible(false);
                }
            });

            revealer
        };

        #[cfg(feature = "layer-shell")]
        let animation_ms = cfg.dropdown_animation_ms;

        drop(cfg);

        Self {
            window,
            visible: RefCell::new(false),

            #[cfg(feature = "layer-shell")]
            target_height,
            #[cfg(feature = "layer-shell")]
            animation_ms,
            #[cfg(feature = "layer-shell")]
            animation_generation: Rc::new(Cell::new(0)),

            #[cfg(feature = "kwin")]
            revealer,

            last_keypress: Rc::new(Cell::new(None)),
            overlay,
            paned,
            stack,
            manager,
            notification_store,
            sidebar,
        }
    }

    pub fn window(&self) -> &ApplicationWindow {
        &self.window
    }

    pub fn visible(&self) -> Ref<'_, bool> {
        self.visible.borrow()
    }

    pub fn record_keypress(&self) {
        self.last_keypress.set(Some(Instant::now()));
    }

    pub fn had_recent_keypress(&self) -> bool {
        self.last_keypress.get()
            .map(|t| t.elapsed() < std::time::Duration::from_millis(500))
            .unwrap_or(false)
    }

    pub fn show(&self) {
        #[cfg(feature = "layer-shell")]
        {
            if !self.window.is_visible() {
                self.window.set_opacity(0.0);
                crate::layer_shell::set_top_margin(&self.window, -self.target_height);
                self.window.set_visible(true);
                self.window.present();
            }

            self.animate(true);
        }

        #[cfg(feature = "kwin")]
        {
            if !self.window.is_visible() {
                self.window.set_visible(true);
                self.window.present();
            }

            self.revealer.set_reveal_child(true);
        }

        *self.visible.borrow_mut() = true;

        if let Some(term) = self.manager.borrow().active_terminal_vte() {
            term.grab_focus();
        }
    }

    /// Present the window off-screen without animating — used to start quake mode hidden.
    pub fn present_hidden(&self) {
        #[cfg(feature = "layer-shell")]
        {
            self.window.set_opacity(0.0);
            crate::layer_shell::set_top_margin(&self.window, -self.target_height);
            self.window.set_visible(true);
            self.window.present();
        }

        // KWin: window starts invisible, first toggle will show it — nothing to do
    }

    pub fn hide(&self) {
        if *self.visible.borrow() {
            #[cfg(feature = "layer-shell")]
            self.animate(false);

            #[cfg(feature = "kwin")]
            self.revealer.set_reveal_child(false);

            *self.visible.borrow_mut() = false;
        }
    }

    pub fn toggle(&self) {
        if *self.visible.borrow() {
            self.hide();
        } else {
            self.show();
        }
    }

    #[cfg(feature = "layer-shell")]
    fn animate(&self, opening: bool) {
        let generation = self.animation_generation.get().wrapping_add(1);
        self.animation_generation.set(generation);

        let target = self.target_height;
        let duration_ms = self.animation_ms as f64;
        let animation_generation = self.animation_generation.clone();
        let start_time: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
        let window = self.window.clone();

        self.window.add_tick_callback(move |_widget, _clock| {
            if animation_generation.get() != generation {
                return glib::ControlFlow::Break;
            }

            let now = Instant::now();
            let start = start_time.get().unwrap_or_else(|| {
                start_time.set(Some(now));
                now
            });

            let elapsed_ms = now.duration_since(start).as_secs_f64() * 1000.0;
            let progress = (elapsed_ms / duration_ms).clamp(0.0, 1.0);

            // Ease-out cubic
            let eased = 1.0 - (1.0 - progress).powi(3);

            let (margin, opacity) = if opening {
                let m = -target + (target as f64 * eased) as i32;
                (m, eased)
            } else {
                let m = -(target as f64 * eased) as i32;
                (m, 1.0 - eased)
            };

            crate::layer_shell::set_top_margin(&window, margin);
            window.set_opacity(opacity);

            if progress >= 1.0 {
                if opening {
                    crate::layer_shell::set_top_margin(&window, 0);
                    window.set_opacity(1.0);
                } else {
                    crate::layer_shell::set_top_margin(&window, -target);
                    window.set_opacity(0.0);
                    window.set_visible(false);
                }

                return glib::ControlFlow::Break;
            }

            glib::ControlFlow::Continue
        });
    }
}
