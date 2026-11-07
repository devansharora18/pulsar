//! User configuration: `~/.config/pulsar/config.toml`, hot-reloadable.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn default_refresh_ms() -> u64 {
    1000
}
fn default_theme() -> String {
    "catppuccin-mocha".to_string()
}
fn default_history_len() -> usize {
    120
}
fn default_smoothing() -> u8 {
    75
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 500..=5000 ms.
    pub refresh_ms: u64,
    pub theme: String,
    /// 0..=90: how much of the previous value to keep (higher = smoother).
    pub smoothing: u8,
    /// Overrides the RAPL/TDP probe when set.
    pub tdp_w: Option<f64>,
    /// 10..=600 samples.
    pub history_len: usize,
    pub hide_kernel_threads: bool,
    /// Ask for confirmation before killing / privileged actions.
    pub confirm_kill: bool,
    /// Prefix tabs/header with Nerd Font glyphs.
    pub nerd_icons: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            refresh_ms: default_refresh_ms(),
            theme: default_theme(),
            smoothing: default_smoothing(),
            tdp_w: None,
            history_len: default_history_len(),
            hide_kernel_threads: false,
            confirm_kill: true,
            nerd_icons: true,
        }
    }
}

impl Config {
    /// Clamp to safe values; called after parse and after a hot-reload.
    pub fn sanitize(mut self) -> Self {
        self.refresh_ms = self.refresh_ms.clamp(500, 5000);
        self.smoothing = self.smoothing.min(90);
        self.history_len = self.history_len.clamp(10, 600);
        if self.theme.trim().is_empty() {
            self.theme = default_theme();
        }
        if let Some(tdp) = self.tdp_w {
            self.tdp_w = Some(tdp.clamp(5.0, 500.0));
        }
        self
    }

    /// EMA weight for a new sample: higher `smoothing` means a slower-moving average.
    pub fn ema_alpha(&self) -> f64 {
        (100.0 - self.smoothing as f64) / 100.0
    }

    pub fn from_toml(text: &str) -> Self {
        toml::from_str::<Config>(text)
            .unwrap_or_default()
            .sanitize()
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pulsar")
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
        assert_eq!(c.smoothing, 75);
        assert_eq!(c.history_len, 120);
        assert!((c.ema_alpha() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let c = Config::from_toml("theme = \"nord\"\nrefresh_ms = 2000\n");
        assert_eq!(c.theme, "nord");
        assert_eq!(c.refresh_ms, 2000);
        assert_eq!(c.smoothing, 75); // default
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let c = Config::from_toml("refresh_ms = 10\nsmoothing = 200\nhistory_len = 1\n");
        assert_eq!(c.refresh_ms, 500);
        assert_eq!(c.smoothing, 90);
        assert_eq!(c.history_len, 10);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        let c = Config::from_toml("this is not = = toml");
        assert_eq!(c.refresh_ms, 1000);
    }

    #[test]
    fn old_desktop_keys_are_ignored() {
        // Configs written by the desktop build must still load.
        let c = Config::from_toml("opacity = 0.9\nfont_size = 14\nblur = true\ntheme = \"nord\"\n");
        assert_eq!(c.theme, "nord");
    }

    #[test]
    fn smoothing_maps_to_alpha() {
        let c = Config {
            smoothing: 0,
            ..Default::default()
        };
        assert!((c.ema_alpha() - 1.0).abs() < 1e-9);
        let c = Config {
            smoothing: 90,
            ..Default::default()
        };
        assert!((c.ema_alpha() - 0.10).abs() < 1e-9);
    }
}
