use std::cell::Cell;
use std::env;

use gtk4::gdk;
use gtk4::glib;
use gtk4::gio;
use gtk4::pango;
use gtk4::prelude::*;
use vte4::prelude::*;
use vte4::{Terminal, PtyFlags};
use gtk4::GestureClick;

use crate::config::Config;
use crate::theme::{self, ColorScheme};

const PCRE2_MULTILINE: u32 = 0x00000400;
const URL_REGEX: &str = r"(?:(?:https?|ftp|file|mailto)://|www\.)[-a-zA-Z0-9+&@#/%?=~_|!:,.;]*[-a-zA-Z0-9+&@#/%=~_|]";

pub struct VteTerminal {
    container: gtk4::Box,
    terminal: Terminal,
    spawned: Cell<bool>,
}

impl VteTerminal {
    pub fn new_with_config(config: &Config) -> Self {
        let scheme = theme::get_scheme(&config.color_scheme);

        let terminal = Terminal::builder()
            .scrollback_lines(config.scrollback_lines)
            .scroll_on_keystroke(true)
            .audible_bell(false)
            .bold_is_bright(true)
            .build();

        let font = pango::FontDescription::from_string(&config.font_description());
        terminal.set_font(Some(&font));

        Self::apply_colors(&terminal, scheme);
        Self::setup_shift_enter(&terminal);
        Self::setup_url_matching(&terminal);
        Self::setup_ctrl_click(&terminal);

        let scrollbar = gtk4::Scrollbar::new(
            gtk4::Orientation::Vertical,
            terminal.vadjustment().as_ref(),
        );

        let container = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        container.set_margin_start(2);
        container.set_margin_end(2);
        terminal.set_hexpand(true);
        terminal.set_vexpand(true);
        container.append(&terminal);
        container.append(&scrollbar);

        Self { container, terminal, spawned: Cell::new(false) }
    }

    fn apply_colors(terminal: &Terminal, scheme: &ColorScheme) {
        let fg = gdk::RGBA::parse(scheme.terminal_fg).expect("valid fg color");
        let bg = gdk::RGBA::parse(scheme.terminal_bg).expect("valid bg color");

        let palette: Vec<gdk::RGBA> = scheme.palette.iter()
            .map(|c| gdk::RGBA::parse(*c).expect("valid palette color"))
            .collect();

        let palette_refs: Vec<&gdk::RGBA> = palette.iter().collect();
        terminal.set_colors(Some(&fg), Some(&bg), &palette_refs);
    }

    fn setup_shift_enter(terminal: &Terminal) {
        let key_controller = gtk4::EventControllerKey::new();
        let terminal_clone = terminal.clone();

        key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
            if keyval == gdk::Key::Return && state.contains(gdk::ModifierType::SHIFT_MASK) {
                terminal_clone.feed_child(b"\x1b[13;2u");
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });

        terminal.add_controller(key_controller);
    }

    fn setup_url_matching(terminal: &Terminal) {
        terminal.set_allow_hyperlink(true);

        let regex = vte4::Regex::for_match(URL_REGEX, PCRE2_MULTILINE)
            .expect("valid URL regex");

        let tag = terminal.match_add_regex(&regex, 0);
        terminal.match_set_cursor_name(tag, "pointer");
    }

    fn setup_ctrl_click(terminal: &Terminal) {
        let gesture = GestureClick::new();
        gesture.set_button(1);

        let term = terminal.clone();
        gesture.connect_released(move |gesture, _n_press, x, y| {
            let state = gesture.current_event_state();

            if !state.contains(gdk::ModifierType::CONTROL_MASK) {
                return;
            }

            if let Some(url) = Self::check_url_at(&term, x, y) {
                gesture.set_state(gtk4::EventSequenceState::Claimed);

                if let Err(e) = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>) {
                    eprintln!("Failed to open URL: {e}");
                }
            }
        });

        terminal.add_controller(gesture);
    }

    /// Check for a URL at the given coordinates (in terminal widget space).
    /// Checks OSC 8 hyperlinks first, then regex matches.
    pub fn check_url_at(terminal: &Terminal, x: f64, y: f64) -> Option<String> {
        if let Some(url) = terminal.check_hyperlink_at(x, y) {
            return Some(url.to_string());
        }

        let (matched, _tag) = terminal.check_match_at(x, y);

        matched.map(|s| {
            let s = s.to_string();

            if s.starts_with("www.") {
                format!("https://{s}")
            } else {
                s
            }
        })
    }

    pub fn needs_spawn(&self) -> bool {
        !self.spawned.get()
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    pub fn spawn_shell(&self, working_directory: Option<&str>, extra_env: &[(&str, &str)]) {
        self.spawned.set(true);
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

        let mut envv: Vec<String> = env::vars()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();

        for (key, value) in extra_env {
            envv.retain(|e| !e.starts_with(&format!("{key}=")));
            envv.push(format!("{key}={value}"));
        }

        let envv_refs: Vec<&str> = envv.iter().map(|s| s.as_str()).collect();

        self.terminal.spawn_async(
            PtyFlags::DEFAULT,
            working_directory,
            &[&shell],
            &envv_refs,
            glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            gio::Cancellable::NONE,
            |result| match result {
                Ok(_pid) => {}
                Err(e) => eprintln!("Failed to spawn shell: {e}"),
            },
        );
    }

}
