mod app;
mod app_state;
mod claude;
mod cli;
mod config;
mod dropdown;
mod git;
mod notifications;
mod session;
mod sidebar;
mod terminal;
mod theme;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::glib;
use gtk4::Application;

use app_state::AppState;

const APP_ID: &str = "com.asermax.seemux";

thread_local! {
    static APP_STATE: RefCell<Option<Rc<AppState>>> = RefCell::new(None);
}

fn main() -> glib::ExitCode {
    if !cli::handle_args() {
        return glib::ExitCode::SUCCESS;
    }

    let application = Application::builder()
        .application_id(APP_ID)
        .build();

    application.connect_startup(|_| {
        let state = Rc::new(AppState::new());
        APP_STATE.with(|s| *s.borrow_mut() = Some(state));
    });

    application.connect_activate(|app| {
        APP_STATE.with(|s| {
            if let Some(state) = s.borrow().as_ref() {
                app::build_window(app, state);
            }
        });
    });

    application.run()
}
