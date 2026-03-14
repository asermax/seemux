use std::cell::Cell;
use std::env;

use gtk4::glib;
use gtk4::gio;
use gtk4::pango;
use gtk4::prelude::*;
use vte4::prelude::*;
use vte4::{Terminal, PtyFlags};

use crate::config::Config;
use crate::theme::{self, ColorScheme};

pub struct VteTerminal {
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

        Self { terminal, spawned: Cell::new(false) }
    }

    fn apply_colors(terminal: &Terminal, scheme: &ColorScheme) {
        let fg = gtk4::gdk::RGBA::parse(scheme.terminal_fg).expect("valid fg color");
        let bg = gtk4::gdk::RGBA::parse(scheme.terminal_bg).expect("valid bg color");

        let palette: Vec<gtk4::gdk::RGBA> = scheme.palette.iter()
            .map(|c| gtk4::gdk::RGBA::parse(*c).expect("valid palette color"))
            .collect();

        let palette_refs: Vec<&gtk4::gdk::RGBA> = palette.iter().collect();
        terminal.set_colors(Some(&fg), Some(&bg), &palette_refs);
    }

    pub fn needs_spawn(&self) -> bool {
        !self.spawned.get()
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.terminal.upcast_ref()
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

    pub fn connect_child_exited<F: Fn(i32) + 'static>(&self, f: F) {
        self.terminal.connect_child_exited(move |_term, status| {
            f(status);
        });
    }

    pub fn connect_title_changed<F: Fn(&str) + 'static>(&self, f: F) {
        self.terminal.connect_window_title_changed(move |term| {
            if let Some(title) = term.window_title() {
                f(&title);
            }
        });
    }

    pub fn connect_cwd_changed<F: Fn(Option<String>) + 'static>(&self, f: F) {
        self.terminal.connect_current_directory_uri_changed(move |term| {
            let path = term.current_directory_uri()
                .and_then(|uri| {
                    uri.strip_prefix("file://")
                        .map(|p| p.to_string())
                        .or_else(|| Some(uri.to_string()))
                });

            f(path);
        });
    }
}
