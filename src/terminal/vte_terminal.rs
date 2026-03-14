use std::env;

use gtk4::glib;
use gtk4::gio;
use gtk4::pango;
use gtk4::prelude::*;
use vte4::prelude::*;
use vte4::{Terminal, PtyFlags};

use std::cell::Cell;

pub struct VteTerminal {
    terminal: Terminal,
    spawned: Cell<bool>,
}

impl VteTerminal {
    pub fn new() -> Self {
        let terminal = Terminal::builder()
            .scrollback_lines(10000)
            .scroll_on_keystroke(true)
            .audible_bell(false)
            .bold_is_bright(true)
            .build();

        let font = pango::FontDescription::from_string("Monospace 13");
        terminal.set_font(Some(&font));

        let fg = gtk4::gdk::RGBA::parse("#cdd6f4").expect("valid color");
        let bg = gtk4::gdk::RGBA::parse("#1e1e2e").expect("valid color");
        terminal.set_color_foreground(&fg);
        terminal.set_color_background(&bg);

        Self { terminal, spawned: Cell::new(false) }
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

        // Build environment: inherit current env + add extras
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
}
