pub struct ColorScheme {
    pub name: &'static str,

    // Terminal colors
    pub terminal_fg: &'static str,
    pub terminal_bg: &'static str,
    pub palette: [&'static str; 16],

    // UI colors
    pub window_bg: &'static str,
    pub sidebar_bg: &'static str,
    pub accent: &'static str,
    pub text_primary: &'static str,
    pub text_secondary: &'static str,
    pub text_muted: &'static str,
    pub separator: &'static str,
    pub surface_hover: &'static str,
    pub surface_active: &'static str,

    // Status colors
    pub status_running: &'static str,
    pub status_needs_input: &'static str,
    pub status_error: &'static str,
    pub status_completed: &'static str,
    pub status_idle: &'static str,
}

pub static CATPPUCCIN_MOCHA: ColorScheme = ColorScheme {
    name: "catppuccin-mocha",

    terminal_fg: "#cdd6f4",
    terminal_bg: "#1e1e2e",
    palette: [
        "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af",
        "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de",
        "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af",
        "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8",
    ],

    window_bg: "#1e1e2e",
    sidebar_bg: "#181825",
    accent: "#89b4fa",
    text_primary: "#cdd6f4",
    text_secondary: "#a6adc8",
    text_muted: "#585b70",
    separator: "#313244",
    surface_hover: "rgba(255, 255, 255, 0.05)",
    surface_active: "rgba(255, 255, 255, 0.08)",

    status_running: "#89b4fa",
    status_needs_input: "#fab387",
    status_error: "#f38ba8",
    status_completed: "#a6e3a1",
    status_idle: "#6c7086",
};

pub static DRACULA: ColorScheme = ColorScheme {
    name: "dracula",

    terminal_fg: "#f8f8f2",
    terminal_bg: "#282a36",
    palette: [
        "#21222c", "#ff5555", "#50fa7b", "#f1fa8c",
        "#bd93f9", "#ff79c6", "#8be9fd", "#f8f8f2",
        "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5",
        "#d6acff", "#ff92df", "#a4ffff", "#ffffff",
    ],

    window_bg: "#282a36",
    sidebar_bg: "#21222c",
    accent: "#bd93f9",
    text_primary: "#f8f8f2",
    text_secondary: "#bfbfbf",
    text_muted: "#6272a4",
    separator: "#44475a",
    surface_hover: "rgba(255, 255, 255, 0.05)",
    surface_active: "rgba(255, 255, 255, 0.1)",

    status_running: "#bd93f9",
    status_needs_input: "#ffb86c",
    status_error: "#ff5555",
    status_completed: "#50fa7b",
    status_idle: "#6272a4",
};

static SCHEMES: &[&ColorScheme] = &[&CATPPUCCIN_MOCHA, &DRACULA];

pub fn get_scheme(name: &str) -> &'static ColorScheme {
    SCHEMES.iter()
        .find(|s| s.name == name)
        .unwrap_or(&&CATPPUCCIN_MOCHA)
}

pub fn generate_css(s: &ColorScheme) -> String {
    format!(
r#"/* seemux — generated theme: {name} */

window {{
    background-color: {window_bg};
}}

.sidebar {{
    background-color: {sidebar_bg};
    padding: 6px 0;
}}

separator {{
    background-color: {separator};
    min-width: 1px;
    min-height: 1px;
}}

paned > separator {{
    min-width: 2px;
    background-color: {separator};
}}

paned > separator:hover {{
    background-color: {text_muted};
}}

.tab-row {{
    padding: 8px 12px;
    border-radius: 6px;
    margin: 2px 6px;
    transition: background-color 150ms ease;
}}

.tab-row:hover {{
    background-color: {surface_hover};
}}

.tab-row.active {{
    background-color: {surface_active};
}}

.tab-row .active-indicator {{
    background-color: transparent;
    min-width: 3px;
    border-radius: 2px;
    transition: background-color 200ms ease;
}}

.tab-row.active .active-indicator {{
    background-color: {accent};
}}

.tab-title {{
    color: {text_primary};
    font-size: 13px;
    font-weight: 500;
}}

.tab-row:not(.active) .tab-title {{
    color: {text_secondary};
}}

.tab-branch {{
    font-size: 10px;
    color: {text_muted};
}}

.tab-close-btn {{
    opacity: 0;
    padding: 2px;
    min-width: 20px;
    min-height: 20px;
    border-radius: 4px;
    border: none;
    background: none;
    transition: opacity 150ms ease, background-color 150ms ease;
}}

.tab-row:hover .tab-close-btn {{
    opacity: 0.5;
}}

.tab-close-btn:hover {{
    opacity: 1;
    background-color: {surface_hover};
}}

.notification-badge {{
    background-color: {accent};
    color: {sidebar_bg};
    font-size: 9px;
    font-weight: 700;
    min-width: 16px;
    min-height: 16px;
    border-radius: 8px;
    padding: 0 4px;
}}

.tab-row.active .notification-badge {{
    background-color: rgba(255, 255, 255, 0.25);
    color: {text_primary};
}}

.status-pill {{
    font-size: 10px;
    font-weight: 500;
    padding: 0;
}}

.status-pill--running {{
    color: {status_running};
}}

.status-pill--needs-input {{
    color: {status_needs_input};
}}

.status-pill--error {{
    color: {status_error};
}}

.status-pill--completed {{
    color: {status_completed};
}}

.status-pill--idle {{
    color: {status_idle};
}}

.tab-notification-preview {{
    color: {text_muted};
    font-size: 11px;
    font-style: italic;
}}

.tab-rename-entry {{
    padding: 0 4px;
    min-height: 20px;
    font-size: 13px;
    background-color: {separator};
    color: {text_primary};
    border: 1px solid {accent};
    border-radius: 4px;
}}

.new-tab-btn {{
    margin: 4px 8px 8px 8px;
    padding: 6px;
    border-radius: 6px;
    border: 1px dashed {text_muted};
    color: {text_muted};
    background: none;
    transition: background-color 150ms ease, color 150ms ease, border-color 150ms ease;
}}

.new-tab-btn:hover {{
    background-color: {surface_hover};
    color: {text_primary};
    border-color: {text_secondary};
}}

list row {{
    background: none;
    padding: 0;
}}

list row:selected {{
    background: none;
}}
"#,
        name = s.name,
        window_bg = s.window_bg,
        sidebar_bg = s.sidebar_bg,
        separator = s.separator,
        text_muted = s.text_muted,
        surface_hover = s.surface_hover,
        surface_active = s.surface_active,
        accent = s.accent,
        text_primary = s.text_primary,
        text_secondary = s.text_secondary,
        status_running = s.status_running,
        status_needs_input = s.status_needs_input,
        status_error = s.status_error,
        status_completed = s.status_completed,
        status_idle = s.status_idle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_scheme_returns_catppuccin_by_default() {
        let scheme = get_scheme("nonexistent");
        assert_eq!(scheme.name, "catppuccin-mocha");
    }

    #[test]
    fn get_scheme_returns_dracula() {
        let scheme = get_scheme("dracula");
        assert_eq!(scheme.name, "dracula");
        assert_eq!(scheme.terminal_bg, "#282a36");
    }

    #[test]
    fn generate_css_contains_scheme_colors() {
        let css = generate_css(&DRACULA);
        assert!(css.contains("#282a36")); // window_bg
        assert!(css.contains("#bd93f9")); // accent
        assert!(css.contains("#ff5555")); // status_error
    }
}
