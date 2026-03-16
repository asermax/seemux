use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::CreateSessionOptions;
use futures_util::StreamExt;
use gtk4::glib;

/// Best-effort registration of a global "toggle-dropdown" shortcut via the
/// XDG Desktop Portal GlobalShortcuts API.  Silently no-ops if the portal
/// is unavailable (older compositors, missing portal backend, etc.).
pub fn register_toggle(on_toggle: impl Fn() + 'static) {
    glib::spawn_future_local(async move {
        let Ok(proxy) = GlobalShortcuts::new().await else {
            eprintln!("seemux: global shortcuts portal not available");
            return;
        };

        let Ok(session) = proxy.create_session(CreateSessionOptions::default()).await else {
            eprintln!("seemux: failed to create global shortcuts session");
            return;
        };

        let shortcut = NewShortcut::new("toggle-dropdown", "Toggle Dropdown Terminal");

        if proxy
            .bind_shortcuts(&session, &[shortcut], None, BindShortcutsOptions::default())
            .await
            .is_err()
        {
            eprintln!("seemux: failed to bind global shortcuts");
            return;
        }

        // Listen for activations — this stream runs for the lifetime of the session
        let Ok(mut stream) = proxy.receive_activated().await else {
            eprintln!("seemux: failed to listen for shortcut activations");
            return;
        };

        while let Some(activated) = stream.next().await {
            if activated.shortcut_id() == "toggle-dropdown" {
                on_toggle();
            }
        }
    });
}
