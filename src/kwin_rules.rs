//! KWin rules management for the dropdown window on KDE Plasma.
//!
//! Installs/removes a KWin window rule in `~/.config/kwinrulesrc` that keeps
//! the dropdown window above others, positions it at the top of the screen,
//! and skips the taskbar/pager. Rules are applied by calling `dbus-send` to
//! trigger a KWin reconfigure.

use std::fs;
use std::path::PathBuf;

const RULE_ID: &str = "a1b2c3d4-seemux-drop-down0";

fn kwinrulesrc_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kwinrulesrc")
}

/// Install (or update) the KWin window rule for the dropdown.
pub fn install_rules(width: i32, height: i32, x: i32, y: i32) {
    let path = kwinrulesrc_path();
    let existing = fs::read_to_string(&path).unwrap_or_default();

    let mut lines = remove_section_lines(&existing);
    ensure_rule_in_general(&mut lines);
    append_rule_section(&mut lines, width, height, x, y);

    let contents = lines.join("\n");

    if contents == existing {
        return;
    }

    if let Err(e) = crate::config::atomic_write(&path, &contents) {
        eprintln!("seemux: failed to write kwinrulesrc: {e}");
        return;
    }

    reconfigure_kwin();
}

/// Remove the KWin window rule for the dropdown (idempotent).
pub fn remove_rules() {
    let path = kwinrulesrc_path();

    let Ok(existing) = fs::read_to_string(&path) else {
        return;
    };

    let mut lines = remove_section_lines(&existing);
    remove_rule_from_general(&mut lines);

    let contents = lines.join("\n");

    if contents == existing {
        return;
    }

    if let Err(e) = crate::config::atomic_write(&path, &contents) {
        eprintln!("seemux: failed to write kwinrulesrc: {e}");
        return;
    }

    reconfigure_kwin();
}

fn reconfigure_kwin() {
    let _ = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.kde.KWin",
            "--type=method_call",
            "/KWin",
            "org.kde.KWin.reconfigure",
        ])
        .spawn();
}

/// Remove the `[RULE_ID]` section from the lines, preserving everything else.
fn remove_section_lines(contents: &str) -> Vec<String> {
    let header = format!("[{RULE_ID}]");
    let mut result = Vec::new();
    let mut inside_section = false;

    for line in contents.lines() {
        if line.trim() == header {
            inside_section = true;
            continue;
        }

        if inside_section && line.starts_with('[') {
            inside_section = false;
        }

        if !inside_section {
            result.push(line.to_string());
        }
    }

    // Remove trailing blank lines
    while result.last().is_some_and(|l| l.trim().is_empty()) {
        result.pop();
    }

    result
}

/// Ensure our rule ID is listed in `[General]`'s `rules` and `count` is correct.
fn ensure_rule_in_general(lines: &mut Vec<String>) {
    let general_idx = lines.iter().position(|l| l.trim() == "[General]");

    if let Some(idx) = general_idx {
        update_general_section(lines, idx, true);
    } else {
        // No [General] section — create one
        if !lines.is_empty() {
            lines.push(String::new());
        }

        lines.push("[General]".to_string());
        lines.push("count=1".to_string());
        lines.push(format!("rules={RULE_ID}"));
    }
}

/// Remove our rule ID from `[General]`'s `rules` and decrement `count`.
fn remove_rule_from_general(lines: &mut Vec<String>) {
    let general_idx = lines.iter().position(|l| l.trim() == "[General]");

    if let Some(idx) = general_idx {
        update_general_section(lines, idx, false);
    }
}

/// Update the `[General]` section to add or remove our rule ID.
fn update_general_section(lines: &mut Vec<String>, general_start: usize, add: bool) {
    // Find the extent of the [General] section
    let section_end = lines.iter()
        .skip(general_start + 1)
        .position(|l| l.starts_with('['))
        .map(|i| i + general_start + 1)
        .unwrap_or(lines.len());

    let mut rules: Vec<String> = Vec::new();
    let mut rules_line_idx = None;
    let mut count_line_idx = None;

    for (i, line) in lines.iter().enumerate().take(section_end).skip(general_start + 1) {
        if let Some(val) = line.strip_prefix("rules=") {
            rules = val.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            rules_line_idx = Some(i);
        } else if line.starts_with("count=") {
            count_line_idx = Some(i);
        }
    }

    // Add or remove our rule ID
    rules.retain(|r| r != RULE_ID);

    if add {
        rules.push(RULE_ID.to_string());
    }

    let count = rules.len();

    // Update or insert the rules line
    let new_rules_line = format!("rules={}", rules.join(","));

    if let Some(idx) = rules_line_idx {
        lines[idx] = new_rules_line;
    } else if count > 0 {
        lines.insert(section_end, new_rules_line);
        // Adjust count_line_idx if it was after the insert point
        if let Some(ref mut ci) = count_line_idx
            && *ci >= section_end
        {
            *ci += 1;
        }
    }

    // Update or insert the count line
    let new_count_line = format!("count={count}");

    if let Some(idx) = count_line_idx {
        lines[idx] = new_count_line;
    } else {
        // Insert right after [General]
        lines.insert(general_start + 1, new_count_line);
    }
}

/// Append the full rule section for the dropdown.
fn append_rule_section(lines: &mut Vec<String>, width: i32, height: i32, x: i32, y: i32) {
    lines.push(String::new());
    lines.push(format!("[{RULE_ID}]"));
    lines.push("Description=seemux dropdown".to_string());
    lines.push("above=true".to_string());
    lines.push("aboverule=2".to_string());
    lines.push("noborder=true".to_string());
    lines.push("noborderrule=2".to_string());
    lines.push(format!("position={x},{y}"));
    lines.push("positionrule=2".to_string());
    lines.push(format!("size={width},{height}"));
    lines.push("sizerule=2".to_string());
    lines.push("skippager=true".to_string());
    lines.push("skippagerrule=2".to_string());
    lines.push("skiptaskbar=true".to_string());
    lines.push("skiptaskbarrule=2".to_string());
    lines.push("title=seemux dropdown".to_string());
    lines.push("titlematch=1".to_string());
    lines.push("types=1".to_string());
    lines.push(String::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_into_empty_file() {
        let lines = remove_section_lines("");
        let mut lines = lines;
        ensure_rule_in_general(&mut lines);
        append_rule_section(&mut lines, 1728, 540, 96, 0);

        let result = lines.join("\n");
        assert!(result.contains("[General]"));
        assert!(result.contains("count=1"));
        assert!(result.contains(&format!("rules={RULE_ID}")));
        assert!(result.contains(&format!("[{RULE_ID}]")));
        assert!(result.contains("above=true"));
        assert!(result.contains("position=96,0"));
        assert!(result.contains("size=1728,540"));
    }

    #[test]
    fn preserves_existing_rules() {
        let existing = "[General]\ncount=1\nrules=other-rule-uuid\n\n[other-rule-uuid]\nDescription=Some other rule\nabove=false\n";
        let mut lines = remove_section_lines(existing);
        ensure_rule_in_general(&mut lines);
        append_rule_section(&mut lines, 100, 200, 50, 0);

        let result = lines.join("\n");
        assert!(result.contains("count=2"));
        assert!(result.contains(&format!("rules=other-rule-uuid,{RULE_ID}")));
        assert!(result.contains("[other-rule-uuid]"));
        assert!(result.contains("Description=Some other rule"));
    }

    #[test]
    fn remove_from_existing() {
        let existing = format!(
            "[General]\ncount=2\nrules=other-rule,{RULE_ID}\n\n[other-rule]\nDescription=Other\n\n[{RULE_ID}]\nDescription=seemux dropdown\nabove=true\n"
        );
        let mut lines = remove_section_lines(&existing);
        remove_rule_from_general(&mut lines);

        let result = lines.join("\n");
        assert!(result.contains("count=1"));
        assert!(result.contains("rules=other-rule"));
        assert!(!result.contains(RULE_ID));
    }

    #[test]
    fn remove_idempotent_when_not_present() {
        let existing = "[General]\ncount=1\nrules=other-rule\n\n[other-rule]\nDescription=Other\n";
        let mut lines = remove_section_lines(existing);
        remove_rule_from_general(&mut lines);

        let result = lines.join("\n");
        assert!(result.contains("count=1"));
        assert!(result.contains("rules=other-rule"));
    }

    #[test]
    fn updates_existing_rule() {
        let existing = format!(
            "[General]\ncount=1\nrules={RULE_ID}\n\n[{RULE_ID}]\nDescription=seemux dropdown\nposition=0,0\nsize=100,100\n"
        );
        let mut lines = remove_section_lines(&existing);
        ensure_rule_in_general(&mut lines);
        append_rule_section(&mut lines, 1920, 540, 0, 0);

        let result = lines.join("\n");
        assert!(result.contains("count=1"));
        assert!(result.contains("size=1920,540"));
        // Old size should be gone
        assert!(!result.contains("size=100,100"));
    }
}
