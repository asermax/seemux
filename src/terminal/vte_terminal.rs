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
// Tolerance in rows for the "at bottom" check in the scroll guard
const SCROLL_BOTTOM_TOLERANCE: f64 = 1.0;
const URL_REGEX: &str = r"(?:(?:https?|ftp|file|mailto)://|www\.|\.\./|\./)[-a-zA-Z0-9+&@#/%?=~_|!:,.;]*[-a-zA-Z0-9+&@#/%=~_|]";

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

        Self::setup_scroll_guard(&terminal, &scrollbar);

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

    /// Prevents VTE's viewport from jumping when CLI apps use cursor-movement
    /// escape sequences (CUU/CSI) to re-render. VTE's `scroll_on_output(false)`
    /// only prevents scrolling on new data, not on cursor movement. This guard
    /// tracks user scroll state and restores the scroll position when VTE tries
    /// to follow the cursor while the user has scrolled up.
    fn setup_scroll_guard(terminal: &Terminal, scrollbar: &gtk4::Scrollbar) {
        let user_scrolled_up = Rc::new(Cell::new(false));
        let frozen_value = Rc::new(Cell::new(0.0_f64));
        let user_interacting = Rc::new(Cell::new(false));
        let scrollbar_active = Rc::new(Cell::new(false));
        let restoring = Rc::new(Cell::new(false));
        let debug_scroll = std::env::var("SEEMUX_DEBUG_SCROLL").is_ok();

        // Detect mouse wheel scrolling (CAPTURE phase so we see it before VTE)
        let scroll_controller = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL,
        );
        scroll_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let ui_for_scroll = user_interacting.clone();
        scroll_controller.connect_scroll(move |_, _, dy| {
            ui_for_scroll.set(true);

            if debug_scroll {
                eprintln!("[scroll-guard] mouse wheel dy={dy:.1}");
            }

            glib::Propagation::Proceed
        });

        terminal.add_controller(scroll_controller);

        // Detect any non-modifier keystroke as user interaction.
        // This covers both explicit scroll keys (Shift+PageUp/Down) and regular
        // typing, which triggers VTE's scroll_on_keystroke behavior.
        let key_scroll_controller = gtk4::EventControllerKey::new();
        key_scroll_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let ui_for_key = user_interacting.clone();
        key_scroll_controller.connect_key_pressed(move |_, keyval, _, _| {
            let is_modifier = matches!(
                keyval,
                gdk::Key::Shift_L | gdk::Key::Shift_R |
                gdk::Key::Control_L | gdk::Key::Control_R |
                gdk::Key::Alt_L | gdk::Key::Alt_R |
                gdk::Key::Super_L | gdk::Key::Super_R |
                gdk::Key::Meta_L | gdk::Key::Meta_R
            );

            if !is_modifier {
                ui_for_key.set(true);
            }

            glib::Propagation::Proceed
        });

        terminal.add_controller(key_scroll_controller);

        // Detect scrollbar drag interaction
        let scrollbar_gesture = gtk4::GestureClick::new();
        scrollbar_gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let sa_for_press = scrollbar_active.clone();
        scrollbar_gesture.connect_pressed(move |_, _, _, _| {
            sa_for_press.set(true);

            if debug_scroll {
                eprintln!("[scroll-guard] scrollbar pressed");
            }
        });

        let sa_for_release = scrollbar_active.clone();
        scrollbar_gesture.connect_released(move |_, _, _, _| {
            sa_for_release.set(false);
        });

        let sa_for_cancel = scrollbar_active.clone();
        scrollbar_gesture.connect_cancel(move |_, _| {
            sa_for_cancel.set(false);
        });

        scrollbar.add_controller(scrollbar_gesture);

        // Core logic: restore scroll position when VTE jumps while user is scrolled up,
        // and snap to bottom when VTE jumps while user was at the bottom.
        let adj = terminal.vadjustment().expect("terminal has vadjustment");

        adj.connect_value_changed(move |adj| {
            if restoring.get() {
                return;
            }

            let value = adj.value();
            let upper = adj.upper();
            let page_size = adj.page_size();
            let at_bottom = value >= upper - page_size - SCROLL_BOTTOM_TOLERANCE;

            let had_user_interaction = user_interacting.replace(false);
            let is_user = had_user_interaction || scrollbar_active.get();

            // User-initiated scroll — update guard state based on position
            if is_user {
                if at_bottom {
                    user_scrolled_up.set(false);
                } else if !scrollbar_active.get()
                    && user_scrolled_up.get()
                    && (value - frozen_value.get()).abs() >= page_size
                {
                    // Large jump during user interaction while scrolled up — VTE cursor
                    // movement coinciding with mouse wheel or keyboard input.
                    // Restore to prevent frozen_value from being overwritten by VTE jump.
                    restoring.set(true);
                    adj.set_value(frozen_value.get());
                    restoring.set(false);

                    if debug_scroll {
                        eprintln!(
                            "[scroll-guard] rejected jump frozen={:.1} (jump tried {:.1})",
                            frozen_value.get(), value,
                        );
                    }
                } else {
                    frozen_value.set(value);
                    user_scrolled_up.set(true);
                }

                return;
            }

            // Non-user scroll (VTE cursor movement / re-render)

            if user_scrolled_up.get() {
                restoring.set(true);
                adj.set_value(frozen_value.get());
                restoring.set(false);

                if debug_scroll {
                    eprintln!(
                        "[scroll-guard] restored frozen={:.1} (vte tried {:.1})",
                        frozen_value.get(), value,
                    );
                }

                return;
            }

            if at_bottom {
                return;
            }

            // VTE jumped away from bottom during re-render — snap back to bottom
            let bottom = (upper - page_size).max(0.0);

            restoring.set(true);
            adj.set_value(bottom);
            restoring.set(false);

            if debug_scroll {
                eprintln!(
                    "[scroll-guard] snap-to-bottom={:.1} (vte jumped to {:.1})",
                    bottom, value,
                );
            }
        });
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
