mod app;
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

use gtk4::prelude::*;
use gtk4::glib;
use gtk4::Application;

const APP_ID: &str = "com.asermax.seemux";

fn main() -> glib::ExitCode {
    // Handle CLI subcommands (e.g. `seemux toggle`)
    if !cli::handle_args() {
        return glib::ExitCode::SUCCESS;
    }

    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(app::build_window);

    app.run()
}
