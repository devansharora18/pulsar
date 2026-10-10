//! Turns a [`Theme`] + [`Config`] into the `:root` CSS custom properties every component
//! consumes. Regenerating this string is how themes hot-reload.

use pulsar_core::config::{Config, Density};
use pulsar_core::theme::Theme;

/// `#rrggbb` + alpha → `rgba(r,g,b,a)`.
pub fn hex_rgba(hex: &str, alpha: f64) -> String {
    let (r, g, b) = Theme::parse_hex(hex);
    format!("rgba({r}, {g}, {b}, {alpha:.3})")
}

pub fn theme_css(theme: &Theme, config: &Config) -> String {
    let density_row = match config.density {
        Density::Compact => "24px",
        Density::Comfy => "30px",
    };
    format!(
        ":root {{
  --bg: {bg};
  --bg-alt: {bg_alt};
  --fg: {fg};
  --fg-dim: {fg_dim};
  --border: {border};
  --accent: {accent};
  --green: {green};
  --yellow: {yellow};
  --orange: {orange};
  --red: {red};
  --blue: {blue};
  --magenta: {magenta};
  --cyan: {cyan};
  --accent-glow: {glow};
  --accent-soft: {soft};
  --grid-line: {grid};
  --radius: {radius}px;
  --gap: {gap}px;
  --font-size: {font_size}px;
  --row-h: {row_h};
  --opacity: {opacity};
  --blur: {blur}px;
  --is-light: {is_light};
}}",
        bg = theme.bg,
        bg_alt = theme.bg_alt,
        fg = theme.fg,
        fg_dim = theme.fg_dim,
        border = theme.border,
        accent = theme.accent,
        green = theme.green,
        yellow = theme.yellow,
        orange = theme.orange,
        red = theme.red,
        blue = theme.blue,
        magenta = theme.magenta,
        cyan = theme.cyan,
        glow = hex_rgba(&theme.accent, 0.35),
        soft = hex_rgba(&theme.accent, 0.12),
        grid = hex_rgba(&theme.border, 0.6),
        radius = config.corner_radius,
        gap = config.gap,
        font_size = config.font_size,
        row_h = density_row,
        opacity = config.opacity,
        blur = if config.blur { 16 } else { 0 },
        is_light = if theme.is_light { 1 } else { 0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_contains_all_tokens() {
        let css = theme_css(&Theme::default(), &Config::default());
        for token in [
            "--bg:",
            "--fg:",
            "--accent:",
            "--border:",
            "--radius:",
            "--font-size:",
            "--row-h:",
            "--accent-glow:",
        ] {
            assert!(css.contains(token), "missing {token}");
        }
        assert!(css.contains("--radius: 12px"));
        assert!(css.contains("--font-size: 13px"));
    }

    #[test]
    fn density_switches_row_height() {
        let c = Config {
            density: Density::Compact,
            ..Default::default()
        };
        assert!(theme_css(&Theme::default(), &c).contains("--row-h: 24px"));
        let c = Config {
            density: Density::Comfy,
            ..Default::default()
        };
        assert!(theme_css(&Theme::default(), &c).contains("--row-h: 30px"));
    }

    #[test]
    fn rgba_alpha() {
        assert_eq!(hex_rgba("#000000", 0.5), "rgba(0, 0, 0, 0.500)");
        assert_eq!(hex_rgba("#ff0000", 1.0), "rgba(255, 0, 0, 1.000)");
    }
}
