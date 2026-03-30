use std::cell::Cell;
use std::env;
use std::rc::Rc;

use gtk4::gdk;
use gtk4::glib;
use gtk4::gio;
use gtk4::pango;
use gtk4::prelude::*;
use vte4::prelude::*;
use vte4::{Terminal, PtyFlags};
use crate::config::Config;
use crate::theme::{self, ColorScheme};

const PCRE2_MULTILINE: u32 = 0x00000400;
const URL_REGEX: &str = r"(?:(?:https?|ftp|file|mailto)://|www\.|\.\./|\./)[-a-zA-Z0-9+&@#/%?=~_|!:,.;]*[-a-zA-Z0-9+&@#/%=~_|]";

pub struct VteTerminal {
    container: gtk4::Box,
    terminal: Terminal,
    spawned: Cell<bool>,
    is_running: Rc<Cell<bool>>,
}

impl VteTerminal {
    pub fn new_with_config(config: &Config) -> Self {
        let scheme = theme::get_scheme(&config.color_scheme);

        let terminal = Terminal::builder()
            .scrollback_lines(config.scrollback_lines)
            .scroll_on_keystroke(true)
            .scroll_on_output(false)
            .audible_bell(false)
            .bold_is_bright(true)
            .build();

        let font = pango::FontDescription::from_string(&config.font_description());
        terminal.set_font(Some(&font));

        Self::apply_colors(&terminal, scheme);
        Self::setup_shift_enter(&terminal);
        Self::setup_url_matching(&terminal);

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

        Self { container, terminal, spawned: Cell::new(false), is_running: Rc::new(Cell::new(false)) }
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

    // -- Public API: operations --

    pub fn grab_focus(&self) {
        self.terminal.grab_focus();
    }

    pub fn feed_child(&self, data: &[u8]) {
        self.terminal.feed_child(data);
    }

    pub fn copy_clipboard(&self) {
        self.terminal.copy_clipboard_format(vte4::Format::Text);
    }

    pub fn paste_clipboard(&self) {
        self.terminal.paste_clipboard();
    }

    pub fn current_directory_uri(&self) -> Option<String> {
        self.terminal.current_directory_uri().map(|s| s.to_string())
    }

    pub fn as_widget(&self) -> &gtk4::Widget {
        self.terminal.upcast_ref()
    }

    // -- Public API: signal callbacks --

    pub fn on_title_changed(&self, cb: impl Fn(Option<String>, Option<String>) + 'static) {
        self.terminal.connect_window_title_changed(move |term| {
            cb(
                term.window_title().map(|s| s.to_string()),
                term.current_directory_uri().map(|s| s.to_string()),
            );
        });
    }

    pub fn on_cwd_changed(&self, cb: impl Fn(Option<String>) + 'static) {
        self.terminal.connect_current_directory_uri_changed(move |term| {
            cb(term.current_directory_uri().map(|s| s.to_string()));
        });
    }

    pub fn on_child_exited(&self, cb: impl Fn(i32) + 'static) {
        self.terminal.connect_child_exited(move |_term, status| {
            cb(status);
        });
    }

    pub fn on_bell(&self, cb: impl Fn() + 'static) {
        self.terminal.connect_bell(move |_term| {
            cb();
        });
    }

    /// Check for a URL at the given coordinates (in terminal widget space).
    /// Checks OSC 8 hyperlinks first, then regex matches.
    pub(crate) fn check_url_at(terminal: &Terminal, x: f64, y: f64) -> Option<String> {
        if let Some(url) = terminal.check_hyperlink_at(x, y) {
            return Some(url.to_string());
        }

        let (matched, _tag) = terminal.check_match_at(x, y);

        matched.map(|s| {
            let s = s.to_string();

            if s.starts_with("www.") {
                format!("https://{s}")
            } else if s.starts_with("./") || s.starts_with("../") {
                let cwd = terminal.current_directory_uri()
                    .and_then(|uri| {
                        let rest = uri.strip_prefix("file://")?;
                        let slash = rest.find('/')?;
                        Some(rest[slash..].to_string())
                    });

                if let Some(cwd) = cwd {
                    let path = std::path::Path::new(&cwd).join(&s);
                    format!("file://{}", path.display())
                } else {
                    s
                }
            } else {
                s
            }
        })
    }

    pub fn is_running(&self) -> &Rc<Cell<bool>> {
        &self.is_running
    }

    pub fn needs_spawn(&self) -> bool {
        !self.spawned.get()
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn spawn_shell(&self, working_directory: Option<&str>, extra_env: &[(&str, &str)]) {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        self.spawn_command(&[&shell], working_directory, extra_env);
    }

    pub fn spawn_command(&self, argv: &[&str], working_directory: Option<&str>, extra_env: &[(&str, &str)]) {
        self.spawned.set(true);

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
            argv,
            &envv_refs,
            glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            gio::Cancellable::NONE,
            |result| match result {
                Ok(_pid) => {}
                Err(e) => eprintln!("Failed to spawn command: {e}"),
            },
        );
    }

}
