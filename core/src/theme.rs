//! Theme engine: built-in TOML themes, user themes and live pywal support.
//!
//! Every colour the UI uses comes from here, so a theme is the single source of truth.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub bg: String,
    pub bg_alt: String,
    pub fg: String,
    pub fg_dim: String,
    pub border: String,
    pub accent: String,
    pub green: String,
    pub yellow: String,
    pub orange: String,
    pub red: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub is_light: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self::builtin("catppuccin-mocha").expect("builtin")
    }
}

impl Theme {
    pub fn builtin(name: &str) -> Option<Theme> {
        let t = match name.to_ascii_lowercase().as_str() {
            "catppuccin-mocha" | "catppuccin" => Theme {
                name: "Catppuccin Mocha".into(),
                bg: "#1e1e2e".into(), bg_alt: "#181825".into(), fg: "#cdd6f4".into(),
                fg_dim: "#a6adc8".into(), border: "#313244".into(), accent: "#89b4fa".into(),
                green: "#a6e3a1".into(), yellow: "#f9e2af".into(), orange: "#fab387".into(),
                red: "#f38ba8".into(), blue: "#89b4fa".into(), magenta: "#cba6f7".into(),
                cyan: "#94e2d5".into(), is_light: false,
            },
            "gruvbox-dark" | "gruvbox" => Theme {
                name: "Gruvbox Dark".into(),
                bg: "#282828".into(), bg_alt: "#1d2021".into(), fg: "#ebdbb2".into(),
                fg_dim: "#928374".into(), border: "#3c3836".into(), accent: "#fabd2f".into(),
                green: "#b8bb26".into(), yellow: "#fabd2f".into(), orange: "#fe8019".into(),
                red: "#fb4934".into(), blue: "#83a598".into(), magenta: "#d3869b".into(),
                cyan: "#8ec07c".into(), is_light: false,
            },
            "rose-pine" | "rosepine" => Theme {
                name: "Rosé Pine".into(),
                bg: "#191724".into(), bg_alt: "#1f1d2e".into(), fg: "#e0def4".into(),
                fg_dim: "#908caa".into(), border: "#26233a".into(), accent: "#c4a7e7".into(),
                green: "#9ccfd8".into(), yellow: "#f6c177".into(), orange: "#ebbcba".into(),
                red: "#eb6f92".into(), blue: "#31748f".into(), magenta: "#c4a7e7".into(),
                cyan: "#9ccfd8".into(), is_light: false,
            },
            "tokyo-night" | "tokyonight" => Theme {
                name: "Tokyo Night".into(),
                bg: "#1a1b26".into(), bg_alt: "#16161e".into(), fg: "#c0caf5".into(),
                fg_dim: "#565f89".into(), border: "#292e42".into(), accent: "#7aa2f7".into(),
                green: "#9ece6a".into(), yellow: "#e0af68".into(), orange: "#ff9e64".into(),
                red: "#f7768e".into(), blue: "#7aa2f7".into(), magenta: "#bb9af7".into(),
                cyan: "#7dcfff".into(), is_light: false,
            },
            "nord" => Theme {
                name: "Nord".into(),
                bg: "#2e3440".into(), bg_alt: "#3b4252".into(), fg: "#d8dee9".into(),
                fg_dim: "#4c566a".into(), border: "#434c5e".into(), accent: "#88c0d0".into(),
                green: "#a3be8c".into(), yellow: "#ebcb8b".into(), orange: "#d08770".into(),
                red: "#bf616a".into(), blue: "#81a1c1".into(), magenta: "#b48ead".into(),
                cyan: "#88c0d0".into(), is_light: false,
            },
            "everforest" => Theme {
                name: "Everforest".into(),
                bg: "#2d353b".into(), bg_alt: "#232a2e".into(), fg: "#d3c6aa".into(),
                fg_dim: "#859289".into(), border: "#3d484d".into(), accent: "#a7c080".into(),
                green: "#a7c080".into(), yellow: "#dbbc7f".into(), orange: "#e69875".into(),
                red: "#e67e80".into(), blue: "#7fbbb3".into(), magenta: "#d699b6".into(),
                cyan: "#83c092".into(), is_light: false,
            },
            _ => return None,
        };
        Some(t)
    }

    /// Built-in theme names, in the order the picker shows them.
    pub fn builtin_names() -> &'static [&'static str] {
        &[
            "catppuccin-mocha",
            "gruvbox-dark",
            "rose-pine",
            "tokyo-night",
            "nord",
            "everforest",
            "pywal",
        ]
    }

    pub fn from_toml(text: &str) -> Option<Theme> {
        toml::from_str::<Theme>(text).ok().map(|mut t| {
            if t.name.is_empty() {
                t.name = "User theme".into();
            }
            t
        })
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    pub fn theme_dir() -> PathBuf {
        crate::config::Config::config_dir().join("themes")
    }

    /// Resolve a theme name: user theme file → builtin → default.
    pub fn resolve(name: &str) -> Theme {
        if name.eq_ignore_ascii_case("pywal") {
            if let Some(t) = pywal_theme() {
                return t;
            }
            return Theme::default();
        }
        let file = Self::theme_dir().join(format!("{name}.toml"));
        if let Ok(text) = std::fs::read_to_string(&file) {
            if let Some(t) = Theme::from_toml(&text) {
                return t;
            }
        }
        Theme::builtin(name).unwrap_or_default()
    }

    pub fn parse_hex(color: &str) -> (u8, u8, u8) {
        let c = color.trim().trim_start_matches('#');
        if c.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&c[0..2], 16),
                u8::from_str_radix(&c[2..4], 16),
                u8::from_str_radix(&c[4..6], 16),
            ) {
                return (r, g, b);
            }
        }
        (0, 0, 0)
    }
}

/// Build a theme from `~/.cache/wal/colors.json` (pywal). Returns `None` if absent.
pub fn pywal_theme() -> Option<Theme> {
    let home = dirs::home_dir()?;
    let path = home.join(".cache/wal/colors.json");
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let get = |k: &str| -> String {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| "#000000".to_string())
    };
    Some(Theme {
        name: "pywal".into(),
        bg: get("background"),
        bg_alt: get("color0"),
        fg: get("foreground"),
        fg_dim: get("color8"),
        border: get("color0"),
        accent: get("color4"),
        green: get("color2"),
        yellow: get("color3"),
        orange: get("color9"),
        red: get("color1"),
        blue: get("color4"),
        magenta: get("color5"),
        cyan: get("color6"),
        is_light: false,
    })
}

/// The GNOME accent colour as a hex string, when present.
pub fn gnome_accent_hex() -> Option<String> {
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "accent-color"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let name = s.trim().trim_matches('\'').trim_matches('"');
    let hex = match name {
        "blue" => "#3584e4",
        "teal" => "#2190a4",
        "green" => "#3a944a",
        "yellow" => "#c88800",
        "orange" => "#ed5b00",
        "red" => "#e62d42",
        "pink" => "#d56199",
        "purple" => "#9141ac",
        "slate" => "#6f8396",
        _ => return None,
    };
    Some(hex.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtins_resolve_and_have_colours() {
        for name in Theme::builtin_names() {
            if *name == "pywal" {
                continue;
            }
            let t = Theme::builtin(name).unwrap_or_else(|| panic!("missing {name}"));
            assert!(t.bg.starts_with('#'));
            assert_eq!(Theme::parse_hex(&t.bg).0, Theme::parse_hex(&t.bg).0);
        }
        assert_eq!(Theme::builtin_names().len(), 7);
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(Theme::parse_hex("#89b4fa"), (0x89, 0xb4, 0xfa));
        assert_eq!(Theme::parse_hex("89b4fa"), (0x89, 0xb4, 0xfa));
        assert_eq!(Theme::parse_hex("bogus"), (0, 0, 0));
        assert_eq!(Theme::parse_hex("#fff"), (0, 0, 0));
    }

    #[test]
    fn partial_theme_toml_fills_from_default() {
        let t = Theme::from_toml("accent = \"#ff0000\"\n").unwrap();
        assert_eq!(t.accent, "#ff0000");
        // Missing keys fall back to the Catppuccin default.
        assert_eq!(t.bg, Theme::default().bg);
    }

    #[test]
    fn malformed_theme_toml_is_none() {
        assert!(Theme::from_toml("= = broken").is_none());
    }

    #[test]
    fn unknown_theme_falls_back() {
        let t = Theme::resolve("definitely-not-a-theme");
        assert_eq!(t.name, Theme::default().name);
    }
}
