use std::cell::Cell;
use std::env;
use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::gdk;
use gtk4::glib;
use gtk4::gio;
use gtk4::pango;
use gtk4::prelude::*;
use regex::Regex;
use vte4::prelude::*;
use vte4::{Format, Terminal, PtyFlags};
use crate::config::Config;
use crate::theme::{self, ColorScheme};

const PCRE2_MULTILINE: u32 = 0x00000400;
const URL_REGEX: &str = r"(?:(?:https?|ftp|file|mailto)://|www\.|\.\./|\./)[-a-zA-Z0-9+&@#/%?=~_|!:,.;]*[-a-zA-Z0-9+&@#/%=~_|]";

fn url_regex_rust() -> &'static Regex {
    static URL_REGEX_RUST: OnceLock<Regex> = OnceLock::new();
    URL_REGEX_RUST.get_or_init(|| Regex::new(URL_REGEX).expect("URL_REGEX is valid Rust regex"))
}

/// Find the URL match whose `[start, end)` byte interval contains `offset`.
///
/// Pure function: no GTK/VTE dependency.
fn find_url_in_logical_line<'a>(line: &'a str, offset: usize, regex: &Regex) -> Option<&'a str> {
    regex
        .find_iter(line)
        .find(|m| m.start() <= offset && offset < m.end())
        .map(|m| m.as_str())
}

/// Walk row indices to find the bounds of the logical line containing `click_row`.
///
/// `is_soft_wrapped(r)` returns `true` iff row `r`'s single-row text does NOT
/// end with `\n` — i.e. row `r` continues into row `r + 1` (soft wrap).
///
/// Pure function: no GTK/VTE dependency.
fn logical_line_bounds(
    click_row: i64,
    buffer_top: i64,
    buffer_last: i64,
    mut is_soft_wrapped: impl FnMut(i64) -> bool,
) -> (i64, i64) {
    let mut top = click_row;
    while top > buffer_top && is_soft_wrapped(top - 1) {
        top -= 1;
    }

    let mut bot = click_row;
    while bot < buffer_last && is_soft_wrapped(bot) {
        bot += 1;
    }

    (top, bot)
}

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
        Self::setup_primary_selection_copy(&terminal);
        Self::setup_middle_click_paste(&terminal);

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

    fn setup_primary_selection_copy(terminal: &Terminal) {
        terminal.connect_selection_changed(|term| {
            if term.has_selection() {
                term.copy_primary();
            }
        });
    }

    fn setup_middle_click_paste(terminal: &Terminal) {
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(2);

        let terminal_clone = terminal.clone();
        gesture.connect_pressed(move |_, _, _, _| {
            terminal_clone.paste_primary();
        });

        terminal.add_controller(gesture);
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
    ///
    /// OSC 8 hyperlinks take priority — VTE associates the URI with each cell
    /// of the anchor span, so this works for both single-row and wrapped
    /// hyperlinks without any reconstruction here.
    ///
    /// For regex-matched URLs, reconstruct the logical line at the click
    /// position by probing rows with `text_range_format(Format::Text, ...)`:
    /// a single-row probe ending in `\n` indicates a hard newline (or the
    /// buffer's last row); no `\n` means soft-wrapped to the next row. Walk
    /// up/down to find the logical-line bounds, concatenate the per-row text
    /// (each trimmed of trailing `\n`) into one `String`, then run the URL
    /// regex on that string and return the match containing the click's
    /// byte offset. This recovers the full URL even when its visual
    /// rendering spans many rows.
    pub(crate) fn check_url_at(terminal: &Terminal, x: f64, y: f64) -> Option<String> {
        if let Some(url) = terminal.check_hyperlink_at(x, y) {
            return Some(url.to_string());
        }

        let column_count = terminal.column_count() as i64;
        let char_w = terminal.char_width() as f64;
        let char_h = terminal.char_height() as f64;

        if column_count <= 0 || char_w <= 0.0 || char_h <= 0.0 {
            return None;
        }

        let vadj = terminal.vadjustment()?;
        let viewport_top = vadj.value().round() as i64;
        let buffer_top = vadj.lower() as i64;
        let buffer_last = (vadj.upper() as i64).saturating_sub(1).max(buffer_top);

        let col = ((x / char_w).floor() as i64).clamp(0, column_count - 1);
        let row =
            (viewport_top + (y / char_h).floor() as i64).clamp(buffer_top, buffer_last);

        let extract_row = |r: i64| -> String {
            let (text, _len) = terminal.text_range_format(Format::Text, r, 0, r, column_count - 1);
            text.map(|g| g.to_string()).unwrap_or_default()
        };

        let (top, bot) = logical_line_bounds(row, buffer_top, buffer_last, |r| {
            !extract_row(r).ends_with('\n')
        });

        let mut logical_line = String::new();
        let mut row_byte_starts: Vec<usize> = Vec::with_capacity((bot - top + 1) as usize);

        for r in top..=bot {
            row_byte_starts.push(logical_line.len());
            let mut row_text = extract_row(r);
            if row_text.ends_with('\n') {
                row_text.pop();
            }
            logical_line.push_str(&row_text);
        }

        let row_index = (row - top) as usize;
        let row_start = row_byte_starts[row_index];
        let prefix_len = if col > 0 {
            let (prefix_text, _len) =
                terminal.text_range_format(Format::Text, row, 0, row, col - 1);
            let mut prefix = prefix_text.map(|g| g.to_string()).unwrap_or_default();
            if prefix.ends_with('\n') {
                prefix.pop();
            }
            prefix.len()
        } else {
            0
        };
        let offset = (row_start + prefix_len).min(logical_line.len());

        let url = find_url_in_logical_line(&logical_line, offset, url_regex_rust())?;

        if url.starts_with("www.") {
            Some(format!("https://{url}"))
        } else if url.starts_with("./") || url.starts_with("../") {
            let cwd = terminal.current_directory_uri().and_then(|uri| {
                let rest = uri.strip_prefix("file://")?;
                let slash = rest.find('/')?;
                Some(rest[slash..].to_string())
            });

            if let Some(cwd) = cwd {
                let path = std::path::Path::new(&cwd).join(url);
                Some(format!("file://{}", path.display()))
            } else {
                Some(url.to_string())
            }
        } else {
            Some(url.to_string())
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn url_re() -> Regex {
        Regex::new(URL_REGEX).expect("URL_REGEX compiles with regex crate")
    }

    #[test]
    fn compiles_url_regex_with_rust_engine() {
        let _ = url_regex_rust();
    }

    #[test]
    fn returns_url_when_offset_inside_match() {
        let re = url_re();
        let line = "see https://example.com/path here";
        let url_start = line.find("https://").unwrap();
        let url_end = line.find(" here").unwrap();
        let mid = url_start + (url_end - url_start) / 2;

        assert_eq!(
            find_url_in_logical_line(line, mid, &re),
            Some("https://example.com/path"),
        );
    }

    #[test]
    fn returns_none_when_offset_outside_match() {
        let re = url_re();
        let line = "see https://example.com/path here";

        assert_eq!(find_url_in_logical_line(line, 0, &re), None);
        assert_eq!(find_url_in_logical_line(line, line.len() - 1, &re), None);
    }

    #[test]
    fn selects_url_at_offset_with_multiple_urls() {
        let re = url_re();
        let line = "https://a.example.com https://b.example.com";
        let a_start = 0;
        let a_end = line.find(' ').unwrap();
        let b_start = a_end + 1;

        assert_eq!(
            find_url_in_logical_line(line, a_start + 5, &re),
            Some("https://a.example.com"),
        );
        assert_eq!(
            find_url_in_logical_line(line, b_start + 5, &re),
            Some("https://b.example.com"),
        );
    }

    #[test]
    fn returns_none_when_offset_in_whitespace_between_urls() {
        let re = url_re();
        let line = "https://a.example.com https://b.example.com";
        let space_offset = line.find(' ').unwrap();

        assert_eq!(find_url_in_logical_line(line, space_offset, &re), None);
    }

    #[test]
    fn returns_url_for_wrapped_logical_line() {
        let re = url_re();
        let long = "a".repeat(150);
        let line = format!("prefix https://example.com/very/long/path/{long}/end suffix");
        let url_start = line.find("https://").unwrap();
        let click_offset = url_start + 100;

        let found = find_url_in_logical_line(&line, click_offset, &re).expect("URL found");
        assert!(found.starts_with("https://example.com/"));
        assert!(found.ends_with("/end"));
        assert!(found.len() > 150);
    }

    #[test]
    fn returns_url_after_multibyte_prefix() {
        let re = url_re();
        let line = "日本語テキスト https://example.com/path text";
        let url_start = line.find("https://").unwrap();

        let found =
            find_url_in_logical_line(&line, url_start + 5, &re).expect("URL found despite CJK");
        assert_eq!(found, "https://example.com/path");
    }

    #[test]
    fn returns_url_for_www_prefix() {
        let re = url_re();
        let line = "go to www.example.com/page now";
        let url_start = line.find("www.").unwrap();

        assert_eq!(
            find_url_in_logical_line(line, url_start + 4, &re),
            Some("www.example.com/page"),
        );
    }

    #[test]
    fn returns_url_for_relative_path() {
        let re = url_re();
        let line = "edit ./src/main.rs please";
        let url_start = line.find("./").unwrap();

        assert_eq!(
            find_url_in_logical_line(line, url_start + 1, &re),
            Some("./src/main.rs"),
        );
    }

    #[test]
    fn returns_none_for_text_with_no_url() {
        let re = url_re();
        let line = "just some text without anything resembling a URL";

        for offset in [0, line.len() / 2, line.len() - 1] {
            assert_eq!(find_url_in_logical_line(line, offset, &re), None);
        }
    }

    fn soft_table(softs: &[(i64, bool)]) -> impl Fn(i64) -> bool + '_ {
        move |r: i64| softs.iter().find(|(rr, _)| *rr == r).map(|(_, s)| *s).unwrap_or(false)
    }

    #[test]
    fn walk_extends_through_soft_wrapped_rows() {
        // Rows 10..=12 form one logical line; row 12 ends with hard newline.
        let softs = [(10, true), (11, true), (12, false)];

        assert_eq!(logical_line_bounds(11, 0, 100, soft_table(&softs)), (10, 12));
        assert_eq!(logical_line_bounds(10, 0, 100, soft_table(&softs)), (10, 12));
        assert_eq!(logical_line_bounds(12, 0, 100, soft_table(&softs)), (10, 12));
    }

    #[test]
    fn walk_stops_at_hard_newline_above() {
        // Row 9 has hard newline (soft=false); rows 10..=11 are soft-wrapped.
        // Click on row 10 must NOT extend up to row 9.
        let softs = [(9, false), (10, true), (11, true), (12, false)];

        let (top, _bot) = logical_line_bounds(10, 0, 100, soft_table(&softs));
        assert_eq!(top, 10);
    }

    #[test]
    fn walk_stops_at_hard_newline_below() {
        // Row 10 has hard newline (soft=false). Click on row 10 must not
        // extend down past row 10 even if row 11 is part of an unrelated
        // logical line.
        let softs = [(10, false), (11, true), (12, false)];

        let (_top, bot) = logical_line_bounds(10, 0, 100, soft_table(&softs));
        assert_eq!(bot, 10);
    }

    #[test]
    fn walk_stops_at_buffer_top() {
        // Click at the very top row; even if row above would be soft-wrapped,
        // the walk must not go below buffer_top.
        let (top, _bot) = logical_line_bounds(0, 0, 100, |_r| true);
        assert_eq!(top, 0);
    }

    #[test]
    fn walk_stops_at_buffer_last() {
        // Click at the very last row; even if soft=true the walk must not
        // go past buffer_last.
        let (_top, bot) = logical_line_bounds(100, 0, 100, |_r| true);
        assert_eq!(bot, 100);
    }
}
