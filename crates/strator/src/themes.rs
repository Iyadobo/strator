//! Themes and backgrounds. Applying one writes the engine config
//! (`config.toml` under the Strator config dir) and dashboard prefs.
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

pub const THEMES: &[&str] = &[
    "catppuccin", "catppuccin-latte", "terminal", "tokyo-night", "tokyo-night-day",
    "dracula", "nord", "gruvbox", "gruvbox-light", "one-dark", "one-light",
    "solarized", "solarized-light", "kanagawa", "kanagawa-lotus", "rose-pine",
    "rose-pine-dawn", "vesper",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Background {
    pub name: &'static str,
    pub panel_bg: &'static str,
    pub sidebar_bg: &'static str,
    pub accent: &'static str,
}

pub const BACKGROUNDS: &[Background] = &[
    Background { name: "midnight", panel_bg: "#0b0e1a", sidebar_bg: "#070912", accent: "#7aa2f7" },
    Background { name: "abyss", panel_bg: "#050807", sidebar_bg: "#030504", accent: "#2dd4bf" },
    Background { name: "ember", panel_bg: "#170c08", sidebar_bg: "#100705", accent: "#ff9e64" },
    Background { name: "forest", panel_bg: "#0a120b", sidebar_bg: "#060a06", accent: "#9ece6a" },
    Background { name: "plum", panel_bg: "#140a1c", sidebar_bg: "#0e0614", accent: "#bb9af7" },
    Background { name: "mono", panel_bg: "#111111", sidebar_bg: "#0a0a0a", accent: "#e0e0e0" },
    Background { name: "paper", panel_bg: "#f4f1ea", sidebar_bg: "#e9e4d8", accent: "#2456a6" },
    Background { name: "none", panel_bg: "", sidebar_bg: "", accent: "" },
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub theme: String,
    pub background: String,
}

impl Prefs {
    pub fn load() -> Self {
        let Ok(path) = prefs_path() else { return Self::default() };
        fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) -> io::Result<()> {
        let path = prefs_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self).unwrap_or_default())
    }
}

fn prefs_path() -> io::Result<PathBuf> {
    Ok(crate::platform::config_dir()?.join("dashboard.json"))
}

pub fn parse_hex(hex: &str) -> ratatui::style::Color {
    use ratatui::style::Color;
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::Reset
}

/// Engine config file the theme is written to.
pub fn engine_config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("STRATOR_CONFIG_PATH") {
        return PathBuf::from(p);
    }
    let base = if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(x)
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default()
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")).unwrap_or_default()
    };
    base.join("strator").join("config.toml")
}

pub fn config_snippet(theme: &str, bg: &Background) -> String {
    let mut out = format!("[theme]\nname = \"{theme}\"\n");
    if bg.name != "none" {
        out.push_str(&format!(
            "\n[theme.custom]\npanel_bg = \"{}\"\nsidebar_bg = \"{}\"\naccent = \"{}\"\n",
            bg.panel_bg, bg.sidebar_bg, bg.accent
        ));
    }
    out
}

/// Replace (or append) the `[theme]` block in an existing config, keeping
/// every other section untouched.
pub fn merge_theme(existing: &str, theme: &str, bg: &Background) -> String {
    let snippet = config_snippet(theme, bg);
    let mut kept: Vec<&str> = Vec::new();
    let mut skip = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skip = trimmed == "[theme]" || trimmed.starts_with("[theme.") || trimmed.starts_with("[theme]");
        }
        if !skip {
            kept.push(line);
        }
    }
    let mut out = kept.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&snippet);
    out
}

pub fn apply(theme: &str, bg_name: &str) -> io::Result<PathBuf> {
    if !THEMES.contains(&theme) {
        return Err(io::Error::other(format!(
            "Unknown theme. Valid: {}",
            THEMES.join(", ")
        )));
    }
    let bg = BACKGROUNDS
        .iter()
        .find(|b| b.name == bg_name)
        .ok_or_else(|| io::Error::other(format!("Unknown background. Valid: {}", BACKGROUNDS.iter().map(|b| b.name).collect::<Vec<_>>().join(", "))))?;
    let path = engine_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    fs::write(&path, merge_theme(&existing, theme, bg))?;
    Prefs { theme: theme.into(), background: bg_name.into() }.save()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snippet_and_merge_preserve_other_sections() {
        let existing = "[ui]\nsidebar = true\n\n[theme]\nname = \"nord\"\n\n[keys]\nquit = \"ctrl+q\"\n";
        let merged = merge_theme(existing, "dracula", &BACKGROUNDS[0]);
        assert!(merged.contains("[ui]"));
        assert!(merged.contains("quit = \"ctrl+q\""));
        assert!(merged.contains("name = \"dracula\""));
        assert!(merged.contains("panel_bg = \"#0b0e1a\""));
        assert!(!merged.contains("\"nord\""));
    }
    #[test]
    fn none_background_writes_theme_only() {
        let none = BACKGROUNDS.iter().find(|b| b.name == "none").unwrap();
        let s = config_snippet("nord", none);
        assert!(s.contains("name = \"nord\""));
        assert!(!s.contains("panel_bg"));
    }
    #[test]
    fn hex_parse() {
        assert_eq!(parse_hex("#ff9e64"), ratatui::style::Color::Rgb(255, 158, 100));
        assert_eq!(parse_hex("bad"), ratatui::style::Color::Reset);
    }
}
