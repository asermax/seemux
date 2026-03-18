use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::Paned;

use crate::config::Config;
use crate::session::manager::SessionManager;

const DEBOUNCE_MS: u64 = 2000;
const SAFETY_NET_SECS: u32 = 30;

pub struct StatePersistence {
    dirty: Cell<bool>,
    debounce_source: Cell<Option<glib::SourceId>>,
    manager: Rc<RefCell<SessionManager>>,
    paned: Paned,
    config: Rc<RefCell<Config>>,
    last_sidebar_width: Cell<i32>,
}

impl StatePersistence {
    pub fn new(
        manager: Rc<RefCell<SessionManager>>,
        paned: Paned,
        config: Rc<RefCell<Config>>,
    ) -> Rc<Self> {
        let initial_width = paned.position();

        let persistence = Rc::new(Self {
            dirty: Cell::new(false),
            debounce_source: Cell::new(None),
            manager,
            paned,
            config,
            last_sidebar_width: Cell::new(initial_width),
        });

        // 30-second safety-net timer — catches any missed mutations
        let weak = Rc::downgrade(&persistence);

        glib::timeout_add_seconds_local(SAFETY_NET_SECS, move || {
            let Some(p) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            p.save_if_dirty();
            glib::ControlFlow::Continue
        });

        persistence
    }

    /// Mark state as dirty and schedule a debounced save.
    pub fn mark_dirty(self: &Rc<Self>) {
        self.dirty.set(true);

        if let Some(source_id) = self.debounce_source.take() {
            source_id.remove();
        }

        let weak = Rc::downgrade(self);

        let source_id = glib::timeout_add_local_once(
            Duration::from_millis(DEBOUNCE_MS),
            move || {
                let Some(p) = weak.upgrade() else { return };

                p.debounce_source.set(None);

                if p.dirty.get() {
                    p.flush();
                }
            },
        );

        self.debounce_source.set(Some(source_id));
    }

    /// Save immediately, cancelling any pending debounce.
    pub fn save_now(&self) {
        self.flush();
    }

    /// Save only if state has been marked dirty (called by safety-net timer).
    fn save_if_dirty(&self) {
        if self.dirty.get() {
            self.flush();
        }
    }

    /// Cancel any pending debounce, write state to disk, and clear the dirty flag.
    fn flush(&self) {
        if let Some(source_id) = self.debounce_source.take() {
            source_id.remove();
        }

        self.manager.borrow().save_state();

        // Only write config when sidebar width actually changed
        let current_width = self.paned.position();

        if current_width != self.last_sidebar_width.get() {
            self.last_sidebar_width.set(current_width);
            let mut cfg = self.config.borrow_mut();
            cfg.sidebar_width = current_width;
            cfg.save();
        }

        self.dirty.set(false);
    }
}
