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

        Self::setup_scroll_guard(&terminal, &scrollbar);

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

    /// Prevents the viewport from jumping when VTE's internal state management
    /// (screen switches, ring growth, set_scrollback_lines) clamps the adjustment
    /// value. Tracks the user's distance from the bottom and restores it when VTE
    /// changes the value while the user is scrolled up.
    fn setup_scroll_guard(terminal: &Terminal, scrollbar: &gtk4::Scrollbar) {
        let offset_from_bottom = Rc::new(Cell::new(0.0_f64));
        let user_interacting = Rc::new(Cell::new(false));
        let scrollbar_active = Rc::new(Cell::new(false));
        let restoring = Rc::new(Cell::new(false));

        // Detect mouse wheel scrolling (CAPTURE phase — before VTE sees it)
        let scroll_controller = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL,
        );
        scroll_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let ui = user_interacting.clone();
        scroll_controller.connect_scroll(move |_, _, _| {
            ui.set(true);
            glib::Propagation::Proceed
        });

        terminal.add_controller(scroll_controller);

        // Detect keyboard-initiated scrolling (Shift+PageUp/Down/Home/End)
        let key_controller = gtk4::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let ui = user_interacting.clone();
        key_controller.connect_key_pressed(move |_, keyval, _, state| {
            if state.contains(gdk::ModifierType::SHIFT_MASK) && matches!(
                keyval,
                gdk::Key::Page_Up | gdk::Key::Page_Down |
                gdk::Key::Home | gdk::Key::End
            ) {
                ui.set(true);
            }

            glib::Propagation::Proceed
        });

        terminal.add_controller(key_controller);

        // Detect scrollbar drag
        let gesture = gtk4::GestureClick::new();
        gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let sa = scrollbar_active.clone();
        gesture.connect_pressed(move |_, _, _, _| { sa.set(true); });

        let sa = scrollbar_active.clone();
        gesture.connect_released(move |_, _, _, _| { sa.set(false); });

        let sa = scrollbar_active.clone();
        gesture.connect_cancel(move |_, _| { sa.set(false); });

        scrollbar.add_controller(gesture);

        // Core logic: restore offset-from-bottom when VTE jumps while scrolled up.
        // We listen to both value_changed (scroll position) and changed (bounds)
        // because VTE may update bounds without emitting value_changed.
        let adj = terminal.vadjustment().expect("terminal has vadjustment");

        let restore_scroll = {
            let offset_from_bottom = offset_from_bottom.clone();
            let restoring = restoring.clone();

            // Returns true when bounds are too small (alt-screen / transient re-render),
            // signalling callers to skip any post-restore work.
            Rc::new(move |adj: &gtk4::Adjustment| -> bool {
                let offset = offset_from_bottom.get();

                if offset <= 0.0 || restoring.get() {
                    return false;
                }

                let max_scroll = adj.upper() - adj.page_size();

                // Skip restore when bounds are too small to fit the offset
                // (alt screen phase). Wait for bounds to return to normal.
                if max_scroll < offset {
                    return true;
                }

                let target = max_scroll - offset;
                let value = adj.value();

                if (target - value).abs() < 1.0 {
                    return false;
                }

                restoring.set(true);
                adj.set_value(target);
                restoring.set(false);

                false
            })
        };

        {
            let restore = restore_scroll.clone();
            let user_interacting = user_interacting.clone();
            let scrollbar_active = scrollbar_active.clone();
            let offset_from_bottom = offset_from_bottom.clone();
            let restoring = restoring.clone();

            adj.connect_value_changed(move |adj| {
                if restoring.get() {
                    return;
                }

                let value = adj.value();
                let max_scroll = adj.upper() - adj.page_size();

                if max_scroll <= 0.0 {
                    return;
                }

                // VTE's scroll_delta is a double that can land fractionally between
                // rows, so allow 1 row of tolerance for the at-bottom check.
                let at_bottom = value >= max_scroll - 1.0;

                let had_interaction = user_interacting.replace(false);
                let is_user = had_interaction || scrollbar_active.get();

                if at_bottom && is_user {
                    offset_from_bottom.set(0.0);
                    return;
                }

                if at_bottom {
                    return;
                }

                if is_user {
                    offset_from_bottom.set(max_scroll - value);
                    return;
                }

                restore(adj);
            });
        }

        // Also restore on bounds changes (VTE may change upper without
        // emitting value_changed, e.g. after screen switch back to normal).
        // After restoring (or confirming the value is already correct), nudge
        // the adjustment to force VTE to re-sync its rendered content — VTE can
        // desync its display from the adjustment value during buffer modifications.
        let restoring_for_changed = restoring.clone();

        adj.connect_changed(move |adj| {
            let bounds_too_small = restore_scroll(adj);

            if !bounds_too_small {
                restoring_for_changed.set(true);
                let v = adj.value();
                adj.set_value(v + 1.0);
                adj.set_value(v);
                restoring_for_changed.set(false);
            }
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
