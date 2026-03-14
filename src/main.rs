mod app;
mod claude;
mod notifications;
mod session;
mod sidebar;
mod terminal;

use gtk4::prelude::*;
use gtk4::glib;
use gtk4::gio;
use gtk4::{Application, CssProvider, gdk::Display};

const APP_ID: &str = "com.asermax.seemux";

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_resource("/com/asermax/seemux/style.css");

    gtk4::style_context_add_provider_for_display(
        &Display::default().expect("Could not connect to a display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn main() -> glib::ExitCode {
    gio::resources_register_include!("seemux.gresource")
        .expect("Failed to register resources");

    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_startup(|_| load_css());
    app.connect_activate(app::build_window);

    app.run()
}
