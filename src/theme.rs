use anyhow::{Context, Result};
use ratatui::style::Color;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    #[serde(default)]
    pub ui: UiTheme,
    #[serde(default)]
    pub timeline: TimelineTheme,
    #[serde(default)]
    pub boxplot: BoxPlotTheme,
    #[serde(default)]
    pub barchart: BarChartTheme,
    #[serde(default)]
    pub statistics: StatisticsTheme,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiTheme {
    #[serde(default = "default_yellow")]
    pub border_focus: String,
    #[serde(default)]
    pub border_unfocus: String,
    #[serde(default = "default_yellow")]
    pub header_fg: String,
    #[serde(default = "default_bold")]
    pub header_modifier: String,
    #[serde(default = "default_col_hl_bg")]
    pub col_highlight_bg: String,
    #[serde(default = "default_white")]
    pub col_highlight_fg: String,
    #[serde(default = "default_row_hl_bg")]
    pub row_highlight_bg: String,
    #[serde(default = "default_white")]
    pub row_highlight_fg: String,
    #[serde(default = "default_row_hl_bg")]
    pub list_highlight_bg: String,
    #[serde(default = "default_red")]
    pub sql_error_border: String,
    #[serde(default = "default_cyan")]
    pub title_fg: String,
    #[serde(default = "default_help_bg")]
    pub help_bar_bg: String,
    #[serde(default = "default_white")]
    pub help_bar_fg: String,
    #[serde(default = "default_red")]
    pub error_fg: String,
    #[serde(default = "default_gray")]
    pub hint_fg: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineTheme {
    #[serde(default = "default_lane_even")]
    pub lane_bg_even: String,
    #[serde(default = "default_lane_odd")]
    pub lane_bg_odd: String,
    #[serde(default = "default_selection_bg")]
    pub selection_bg: String,
    #[serde(default = "default_dark_gray")]
    pub lane_label_fg: String,
    #[serde(default = "default_gray")]
    pub time_axis_fg: String,
    #[serde(default)]
    pub kernel_colors: Vec<String>,
    #[serde(default = "default_magenta")]
    pub memcpy_color: String,
    #[serde(default = "default_yellow")]
    pub memset_color: String,
    #[serde(default = "default_white")]
    pub event_fallback: String,
    #[serde(default = "default_white")]
    pub hover_text_bg: String,
    #[serde(default = "default_black")]
    pub hover_text_fg: String,
    #[serde(default = "default_selection_bg")]
    pub selected_text_bg: String,
    #[serde(default = "default_help_bg")]
    pub info_bar_bg: String,
    #[serde(default = "default_white")]
    pub info_bar_fg: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoxPlotTheme {
    #[serde(default = "default_green")]
    pub whisker_fg: String,
    #[serde(default = "default_green")]
    pub box_fg: String,
    #[serde(default = "default_yellow")]
    pub median_fg: String,
    #[serde(default = "default_gray")]
    pub axis_fg: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BarChartTheme {
    #[serde(default = "default_green")]
    pub bar_fg: String,
    #[serde(default = "default_black")]
    pub bar_value_fg: String,
    #[serde(default = "default_green")]
    pub bar_value_bg: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatisticsTheme {
    #[serde(default = "default_white")]
    pub text_fg: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ui: UiTheme::default(),
            timeline: TimelineTheme::default(),
            boxplot: BoxPlotTheme::default(),
            barchart: BarChartTheme::default(),
            statistics: StatisticsTheme::default(),
        }
    }
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            border_focus: default_yellow(),
            border_unfocus: String::new(),
            header_fg: default_yellow(),
            header_modifier: default_bold(),
            col_highlight_bg: default_col_hl_bg(),
            col_highlight_fg: default_white(),
            row_highlight_bg: default_row_hl_bg(),
            row_highlight_fg: default_white(),
            list_highlight_bg: default_row_hl_bg(),
            sql_error_border: default_red(),
            title_fg: default_cyan(),
            help_bar_bg: default_help_bg(),
            help_bar_fg: default_white(),
            error_fg: default_red(),
            hint_fg: default_gray(),
        }
    }
}

impl Default for TimelineTheme {
    fn default() -> Self {
        Self {
            lane_bg_even: default_lane_even(),
            lane_bg_odd: default_lane_odd(),
            selection_bg: default_selection_bg(),
            lane_label_fg: default_dark_gray(),
            time_axis_fg: default_gray(),
            kernel_colors: default_kernel_colors(),
            memcpy_color: default_magenta(),
            memset_color: default_yellow(),
            event_fallback: default_white(),
            hover_text_bg: default_white(),
            hover_text_fg: default_black(),
            selected_text_bg: default_selection_bg(),
            info_bar_bg: default_help_bg(),
            info_bar_fg: default_white(),
        }
    }
}

impl Default for BoxPlotTheme {
    fn default() -> Self {
        Self {
            whisker_fg: default_green(),
            box_fg: default_green(),
            median_fg: default_yellow(),
            axis_fg: default_gray(),
        }
    }
}

impl Default for BarChartTheme {
    fn default() -> Self {
        Self {
            bar_fg: default_green(),
            bar_value_fg: default_black(),
            bar_value_bg: default_green(),
        }
    }
}

impl Default for StatisticsTheme {
    fn default() -> Self {
        Self {
            text_fg: default_white(),
        }
    }
}

// Default value helpers
fn default_yellow() -> String { "Yellow".into() }
fn default_red() -> String { "Red".into() }
fn default_green() -> String { "Green".into() }
fn default_cyan() -> String { "Cyan".into() }
fn default_magenta() -> String { "Magenta".into() }
fn default_white() -> String { "White".into() }
fn default_black() -> String { "Black".into() }
fn default_gray() -> String { "Gray".into() }
fn default_dark_gray() -> String { "DarkGray".into() }
fn default_bold() -> String { "bold".into() }
fn default_col_hl_bg() -> String { "#005050".into() }
fn default_row_hl_bg() -> String { "DarkGray".into() }
fn default_help_bg() -> String { "DarkGray".into() }
fn default_lane_even() -> String { "#1e1e2a".into() }
fn default_lane_odd() -> String { "#262632".into() }
fn default_selection_bg() -> String { "#3c3c50".into() }
fn default_kernel_colors() -> Vec<String> {
    vec![
        "Cyan".into(), "Green".into(), "Blue".into(), "Red".into(),
        "Magenta".into(), "Yellow".into(), "#ffa500".into(), "#00ced1".into(),
    ]
}

/// Parse a color string: "#RRGGBB", named color, or empty (terminal default).
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(r) = u8::from_str_radix(&hex[0..2], 16) {
                if let Ok(g) = u8::from_str_radix(&hex[2..4], 16) {
                    if let Ok(b) = u8::from_str_radix(&hex[4..6], 16) {
                        return Some(Color::Rgb(r, g, b));
                    }
                }
            }
        }
        return None;
    }
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "lightgray" => Some(Color::Gray),
        "darkgray" | "dark_gray" => Some(Color::DarkGray),
        _ => None,
    }
}

/// Parse a modifier string: "bold", "dim", "italic", "reversed", or empty.
pub fn parse_modifier(s: &str) -> ratatui::style::Modifier {
    let s = s.trim().to_lowercase();
    match s.as_str() {
        "bold" => ratatui::style::Modifier::BOLD,
        "dim" => ratatui::style::Modifier::DIM,
        "italic" => ratatui::style::Modifier::ITALIC,
        "reversed" | "reverse" => ratatui::style::Modifier::REVERSED,
        _ => ratatui::style::Modifier::empty(),
    }
}

/// Load a theme by name from theme directories.
pub fn load_theme(name: &str) -> Result<Theme> {
    let dirs = theme_dirs();
    for dir in &dirs {
        let path = dir.join(format!("{}.toml", name));
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Reading theme file {:?}", path))?;
            let theme: Theme = toml::from_str(&content)
                .with_context(|| format!("Parsing theme file {:?}", path))?;
            return Ok(theme);
        }
    }
    anyhow::bail!("Theme '{}' not found in {:?}", name, dirs)
}

/// Discover all available theme names.
pub fn discover_themes() -> Vec<String> {
    let mut names = Vec::new();
    let dirs = theme_dirs();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs.iter().rev() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "toml") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if seen.insert(stem.to_string()) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    names.sort();
    names
}

/// Return theme directories in priority order (highest first).
fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User theme dir
    if let Some(config) = dirs::config_dir() {
        dirs.push(config.join("nvis").join("themes"));
    }

    // System theme dir (relative to executable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            dirs.push(exe_dir.join("..").join("share").join("nvis").join("themes"));
        }
    }

    dirs
}
