mod app;
mod app_state;
mod cli;
mod config;
mod dropdown;
mod git;
mod global_shortcuts;
mod layer_shell;
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
    let mode = cli::handle_args();

    let quake = matches!(mode, cli::LaunchMode::Quake);

    if matches!(mode, cli::LaunchMode::CommandHandled) {
        return glib::ExitCode::SUCCESS;
    }

    let application = Application::builder()
        .application_id(APP_ID)
        .build();

    application.connect_startup(move |_| {
        let state = Rc::new(AppState::new(quake));

        // Load theme CSS once for the display
        let scheme = theme::get_scheme(&state.config.borrow().color_scheme);
        let css_content = theme::generate_css(scheme);
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(&css_content);
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("Could not connect to a display"),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        APP_STATE.with(|s| *s.borrow_mut() = Some(state));
    });

    application.connect_activate(|app| {
        APP_STATE.with(|s| {
            if let Some(state) = s.borrow().as_ref() {
                if state.quake {
                    app::build_quake_window(app, state);
                } else {
                    app::build_window(app, state);
                }
            }
        });
    });

    // Filter out our custom args so GTK doesn't reject them
    let gtk_args: Vec<String> = std::env::args()
        .filter(|a| a != "--quake")
        .collect();

    application.run_with_args(&gtk_args)
}
