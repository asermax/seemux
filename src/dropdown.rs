use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Orientation, Paned,
    Stack, StackTransitionType, glib,
};

use crate::app_state::AppState;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;

pub struct DropdownWindow {
    window: ApplicationWindow,
    visible: RefCell<bool>,
    target_height: i32,
    animation_ms: u32,
    /// Incremented on each animation start; stale callbacks see a mismatch and stop.
    animation_generation: Rc<Cell<u32>>,
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

        // Layer shell: anchor to top of screen, start off-screen
        crate::layer_shell::setup_dropdown(&window, width, monitor_width, -target_height);

        window.set_default_size(width, target_height);

        // Build full UI
        let sidebar = Rc::new(Sidebar::new());

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

        let manager = SessionManager::new(
            stack,
            sidebar.clone(),
            state.socket_path.clone(),
            config.clone(),
        );

        let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

        // Create first session
        let first_id = manager.borrow_mut().create_session(None, None);
        wire_tab(&sidebar, &manager, &notification_store, &first_id);

        // Wire new tab button
        let mgr = manager.clone();
        let sid = sidebar.clone();
        let notif = notification_store.clone();
        sidebar.connect_new_tab(move || {
            let id = mgr.borrow_mut().create_session(None, None);
            wire_tab(&sid, &mgr, &notif, &id);
        });

        // When all sessions are closed (via child-exited), respawn a new one
        let mgr_empty = manager.clone();
        let sid_empty = sidebar.clone();
        let notif_empty = notification_store.clone();
        manager.borrow_mut().set_on_empty(move || {
            let mgr = mgr_empty.clone();
            let sid = sid_empty.clone();
            let notif = notif_empty.clone();

            glib::idle_add_local_once(move || {
                let id = mgr.borrow_mut().create_session(None, None);
                wire_tab(&sid, &mgr, &notif, &id);
                mgr.borrow().spawn_deferred();
            });
        });

        // Border wrapper
        let content = GtkBox::new(Orientation::Vertical, 0);
        content.add_css_class("dropdown-border");
        content.set_vexpand(true);
        content.append(&paned);

        window.set_child(Some(&content));

        let animation_ms = cfg.dropdown_animation_ms;

        drop(cfg);

        // Spawn shell after layout
        let mgr_spawn = manager.clone();
        glib::idle_add_local_once(move || {
            mgr_spawn.borrow().spawn_deferred();
        });

        Self {
            window,
            visible: RefCell::new(false),
            target_height,
            animation_ms,
            animation_generation: Rc::new(Cell::new(0)),
            manager,
            notification_store,
            sidebar,
        }
    }

    pub fn window(&self) -> &ApplicationWindow {
        &self.window
    }

    pub fn show(&self) {
        self.ensure_window_ready();
        self.animate(true);
        *self.visible.borrow_mut() = true;
    }

    /// Present the window off-screen without animating — used to start quake mode hidden.
    pub fn present_hidden(&self) {
        self.ensure_window_ready();
    }

    fn ensure_window_ready(&self) {
        if !self.window.is_visible() {
            self.window.set_opacity(0.0);
            crate::layer_shell::set_top_margin(&self.window, -self.target_height);
            self.window.set_visible(true);
            self.window.present();
        }
    }

    pub fn toggle(&self) {
        let is_visible = *self.visible.borrow();

        if is_visible {
            self.animate(false);
            *self.visible.borrow_mut() = false;
        } else {
            self.show();
        }
    }

    fn animate(&self, opening: bool) {
        let generation = self.animation_generation.get().wrapping_add(1);
        self.animation_generation.set(generation);

        let target = self.target_height;
        let duration_us = (self.animation_ms as i64) * 1000;
        let animation_generation = self.animation_generation.clone();
        let start_time: Rc<Cell<i64>> = Rc::new(Cell::new(0));
        let window = self.window.clone();

        self.window.add_tick_callback(move |_widget, clock| {
            if animation_generation.get() != generation {
                return glib::ControlFlow::Break;
            }

            let now = clock.frame_time();

            if start_time.get() == 0 {
                start_time.set(now);
            }

            let elapsed = now - start_time.get();
            let progress = (elapsed as f64 / duration_us as f64).clamp(0.0, 1.0);

            // Ease-out quint for smoother deceleration
            let eased = 1.0 - (1.0 - progress).powi(5);

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
                    // Ensure final state is exact
                    crate::layer_shell::set_top_margin(&window, 0);
                    window.set_opacity(1.0);
                } else {
                    crate::layer_shell::set_top_margin(&window, -target);
                    window.set_opacity(0.0);
                }
                return glib::ControlFlow::Break;
            }

            glib::ControlFlow::Continue
        });
    }
}

/// Wire tab click, close button (with last-session guard), child-exited, and focus tracking.
pub fn wire_tab(
    sidebar: &Rc<Sidebar>,
    manager: &Rc<RefCell<SessionManager>>,
    notification_store: &Rc<RefCell<NotificationStore>>,
    session_id: &str,
) {
    let mgr = manager.clone();
    let notif = notification_store.clone();
    sidebar.wire_tab_click(session_id, move |id| {
        if let Ok(mut m) = mgr.try_borrow_mut() {
            m.switch_to(&id);
        }
        notif.borrow_mut().mark_read(&id);
    });

    let mgr = manager.clone();
    sidebar.wire_close_button(session_id, move |id| {
        if mgr.borrow().session_count() > 1 {
            mgr.borrow_mut().destroy_session(&id);
        }
    });

    SessionManager::wire_child_exited(manager, session_id);
    SessionManager::wire_focus_tracking(manager, session_id);
}
