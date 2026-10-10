//! User configuration: `~/.config/pulsar/config.toml`, hot-reloadable.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    Comfy,
}

impl Default for Density {
    fn default() -> Self {
        Density::Comfy
    }
}

fn default_refresh_ms() -> u64 {
    1000
}
fn default_theme() -> String {
    "catppuccin-mocha".to_string()
}
fn default_opacity() -> f64 {
    1.0
}
fn default_radius() -> u32 {
    12
}
fn default_gap() -> u32 {
    10
}
fn default_font_size() -> u32 {
    13
}
fn default_history_len() -> usize {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 500..=5000 ms.
    pub refresh_ms: u64,
    pub theme: String,
    /// 0.5..=1.0 window opacity.
    pub opacity: f64,
    pub blur: bool,
    pub corner_radius: u32,
    pub gap: u32,
    pub font_size: u32,
    pub density: Density,
    /// Overrides the RAPL/TDP probe when set.
    pub tdp_w: Option<f64>,
    /// Ask for confirmation before killing a group.
    pub confirm_kill: bool,
    pub history_len: usize,
    pub hide_kernel_threads: bool,
    /// Show a system tray icon.
    pub tray: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            refresh_ms: default_refresh_ms(),
            theme: default_theme(),
            opacity: default_opacity(),
            blur: false,
            corner_radius: default_radius(),
            gap: default_gap(),
            font_size: default_font_size(),
            density: Density::default(),
            tdp_w: None,
            confirm_kill: true,
            history_len: default_history_len(),
            hide_kernel_threads: false,
            tray: true,
        }
    }
}

impl Config {
    /// Clamp to safe values; called after parse and after a hot-reload.
    pub fn sanitize(mut self) -> Self {
        self.refresh_ms = self.refresh_ms.clamp(500, 5000);
        self.opacity = self.opacity.clamp(0.5, 1.0);
        self.corner_radius = self.corner_radius.min(48);
        self.gap = self.gap.min(48);
        self.font_size = self.font_size.clamp(9, 28);
        self.history_len = self.history_len.clamp(10, 600);
        if self.theme.trim().is_empty() {
            self.theme = default_theme();
        }
        self
    }

    pub fn from_toml(text: &str) -> Self {
        toml::from_str::<Config>(text).unwrap_or_default().sanitize()
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    pub fn config_dir() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("pulsar")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Self {
        match std::fs::read_to_string(Self::config_path()) {
            Ok(text) => Self::from_toml(&text),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(Self::config_path(), self.to_toml())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = Config::default();
        assert_eq!(c.refresh_ms, 1000);
        assert_eq!(c.corner_radius, 12);
        assert_eq!(c.history_len, 120);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let c = Config::from_toml("theme = \"nord\"\nrefresh_ms = 2000\n");
        assert_eq!(c.theme, "nord");
        assert_eq!(c.refresh_ms, 2000);
        assert_eq!(c.font_size, 13); // default
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let c = Config::from_toml("refresh_ms = 10\nopacity = 5.0\nfont_size = 100\nhistory_len = 1\n");
        assert_eq!(c.refresh_ms, 500);
        assert_eq!(c.opacity, 1.0);
        assert_eq!(c.font_size, 28);
        assert_eq!(c.history_len, 10);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        let c = Config::from_toml("this is not = = toml");
        assert_eq!(c.refresh_ms, 1000);
    }
}
