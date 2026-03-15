use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Orientation, Paned, Revealer, RevealerTransitionType,
    Stack, StackTransitionType, glib,
};

use crate::app_state::AppState;
use crate::notifications::NotificationStore;
use crate::session::manager::SessionManager;
use crate::sidebar::Sidebar;
use crate::theme;

pub struct DropdownWindow {
    window: ApplicationWindow,
    revealer: Revealer,
    visible: RefCell<bool>,
}

impl DropdownWindow {
    pub fn new(app: &Application, state: &Rc<AppState>) -> Self {
        let config = state.config.clone();
        let cfg = config.borrow();

        let window = ApplicationWindow::builder()
            .application(app)
            .title("seemux dropdown")
            .decorated(false)
            .default_width(1)
            .default_height(1)
            .build();

        // Load theme CSS for this window
        let scheme = theme::get_scheme(&cfg.color_scheme);
        let css_content = theme::generate_css(scheme);
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(&css_content);
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

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
        let height = (monitor_height as f64 * cfg.dropdown_height_percent as f64 / 100.0) as i32;

        window.set_default_size(width, height);

        // Build full UI inside the revealer
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

        // Create first session
        let first_id = manager.borrow_mut().create_session(None, None);

        // Wire tab click + close + child-exited
        let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

        let mgr = manager.clone();
        let notif = notification_store.clone();
        sidebar.wire_tab_click(&first_id, move |id| {
            if let Ok(mut m) = mgr.try_borrow_mut() {
                m.switch_to(&id);
            }
            notif.borrow_mut().mark_read(&id);
        });

        let mgr = manager.clone();
        sidebar.wire_close_button(&first_id, move |id| {
            mgr.borrow_mut().destroy_session(&id);
        });

        SessionManager::wire_child_exited(&manager, &first_id);
        SessionManager::wire_focus_tracking(&manager, &first_id);

        // Wire new tab button
        let mgr = manager.clone();
        let sid = sidebar.clone();
        let notif2 = notification_store.clone();
        sidebar.connect_new_tab(move || {
            let id = mgr.borrow_mut().create_session(None, None);

            let mgr2 = mgr.clone();
            let notif3 = notif2.clone();
            sid.wire_tab_click(&id, move |id| {
                if let Ok(mut m) = mgr2.try_borrow_mut() {
                    m.switch_to(&id);
                }
                notif3.borrow_mut().mark_read(&id);
            });

            let mgr2 = mgr.clone();
            sid.wire_close_button(&id, move |id| {
                mgr2.borrow_mut().destroy_session(&id);
            });

            SessionManager::wire_child_exited(&mgr, &id);
            SessionManager::wire_focus_tracking(&mgr, &id);
        });

        // Quit-on-empty for dropdown just hides it
        // (don't quit the whole app when dropdown tabs are closed)

        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(cfg.dropdown_animation_ms);
        revealer.set_reveal_child(false);
        revealer.set_child(Some(&paned));
        revealer.set_vexpand(true);

        window.set_child(Some(&revealer));

        drop(cfg);

        // Spawn shell after layout
        let mgr_spawn = manager.clone();
        glib::idle_add_local_once(move || {
            mgr_spawn.borrow().spawn_deferred();
        });

        Self {
            window,
            revealer,
            visible: RefCell::new(false),
        }
    }

    pub fn toggle(&self) {
        let is_visible = *self.visible.borrow();

        if is_visible {
            self.revealer.set_reveal_child(false);

            let window = self.window.clone();
            let duration = self.revealer.transition_duration();
            glib::timeout_add_local_once(
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
}
