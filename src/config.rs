use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font_family: String,
    pub font_size: u32,
    pub scrollback_lines: u32,
    pub sidebar_width: i32,
    pub color_scheme: String,
    pub dropdown_width_percent: u32,
    pub dropdown_height_percent: u32,
    pub dropdown_animation_ms: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_family: "Monospace".to_string(),
            font_size: 13,
            scrollback_lines: 10000,
            sidebar_width: 200,
            color_scheme: "catppuccin-mocha".to_string(),
            dropdown_width_percent: 90,
            dropdown_height_percent: 50,
            dropdown_animation_ms: 500,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();

        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("Failed to parse config: {e}"),
                },
                Err(e) => eprintln!("Failed to read config: {e}"),
            }
        }

        let config = Config::default();
        config.save();
        config
    }

    pub fn save(&self) {
        let path = config_path();

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match toml::to_string_pretty(self) {
            Ok(contents) => {
                if let Err(e) = fs::write(&path, contents) {
                    eprintln!("Failed to write config: {e}");
                }
            }
            Err(e) => eprintln!("Failed to serialize config: {e}"),
        }
    }

    pub fn font_description(&self) -> String {
        format!("{} {}", self.font_family, self.font_size)
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("seemux")
        .join("config.toml")
}

/// Serializable split tree node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SavedSplitNode {
    Leaf { cwd: Option<String> },
    Split {
        orientation: String,
        first: Box<SavedSplitNode>,
        second: Box<SavedSplitNode>,
    },
}

impl Default for SavedSplitNode {
    fn default() -> Self {
        Self::Leaf { cwd: None }
    }
}

/// Session state saved/restored across restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub title: String,
    pub split_tree: SavedSplitNode,
    #[serde(default)]
    pub group_id: String,
    #[serde(default)]
    pub claude_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub sessions: Vec<SavedSession>,
    #[serde(default)]
    pub groups: Vec<SavedGroup>,
    #[serde(default)]
    pub active_session_index: Option<usize>,
}

impl SessionState {
    pub fn load() -> Self {
        let path = state_path();

        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(state) => return state,
                    Err(e) => eprintln!("Failed to parse session state: {e}"),
                },
                Err(e) => eprintln!("Failed to read session state: {e}"),
            }
        }

        Self::default()
    }

    pub fn save(&self) {
        let path = state_path();

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match serde_json::to_string_pretty(self) {
            Ok(contents) => {
                if let Err(e) = fs::write(&path, contents) {
                    eprintln!("Failed to write session state: {e}");
                }
            }
            Err(e) => eprintln!("Failed to serialize session state: {e}"),
        }
    }
}

fn state_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("seemux")
        .join("sessions.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = Config::default();
        assert_eq!(config.font_family, "Monospace");
        assert_eq!(config.dropdown_width_percent, 90);
        assert_eq!(config.dropdown_height_percent, 50);
        assert_eq!(config.dropdown_animation_ms, 500);
        assert_eq!(config.font_size, 13);
        assert_eq!(config.font_description(), "Monospace 13");
    }

    #[test]
    fn config_roundtrip_toml() {
        let config = Config {
            font_family: "JetBrains Mono".to_string(),
            font_size: 14,
            scrollback_lines: 5000,
            sidebar_width: 250,
            color_scheme: "dracula".to_string(),
            dropdown_width_percent: 90,
            dropdown_height_percent: 50,
            dropdown_animation_ms: 200,
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.font_family, "JetBrains Mono");
        assert_eq!(parsed.font_size, 14);
        assert_eq!(parsed.scrollback_lines, 5000);
        assert_eq!(parsed.sidebar_width, 250);
    }

    #[test]
    fn config_partial_toml_uses_defaults() {
        let toml_str = r#"font_size = 16"#;
        let parsed: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(parsed.font_size, 16);
        assert_eq!(parsed.font_family, "Monospace"); // default
        assert_eq!(parsed.scrollback_lines, 10000); // default
    }

    #[test]
    fn session_state_roundtrip() {
        let state = SessionState {
            sessions: vec![
                SavedSession {
                    title: "Tab 1".to_string(),
                    split_tree: SavedSplitNode::Leaf { cwd: Some("/home/user".to_string()) },
                    group_id: "default".to_string(),
                    claude_session_id: Some("abc-123".to_string()),
                },
                SavedSession {
                    title: "Tab 2".to_string(),
                    split_tree: SavedSplitNode::Split {
                        orientation: "horizontal".to_string(),
                        first: Box::new(SavedSplitNode::Leaf { cwd: Some("/tmp".to_string()) }),
                        second: Box::new(SavedSplitNode::Leaf { cwd: None }),
                    },
                    group_id: "group1".to_string(),
                    claude_session_id: None,
                },
            ],
            groups: vec![
                SavedGroup { id: "group1".to_string(), name: "Work".to_string(), collapsed: true },
            ],
            active_session_index: Some(1),
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: SessionState = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.sessions.len(), 2);
        assert_eq!(parsed.sessions[0].title, "Tab 1");
        assert!(matches!(parsed.sessions[0].split_tree, SavedSplitNode::Leaf { .. }));
        assert!(matches!(parsed.sessions[1].split_tree, SavedSplitNode::Split { .. }));
        assert_eq!(parsed.sessions[0].claude_session_id, Some("abc-123".to_string()));
        assert_eq!(parsed.sessions[1].claude_session_id, None);
        assert_eq!(parsed.active_session_index, Some(1));
    }

    #[test]
    fn session_state_backward_compat_missing_claude_session_id() {
        let json = r#"{"sessions":[{"title":"Tab","split_tree":{"Leaf":{"cwd":null}},"group_id":"default"}],"groups":[]}"#;
        let parsed: SessionState = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.sessions[0].claude_session_id, None);
    }
}
