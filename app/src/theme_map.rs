//! Map the TOML [`Theme`](pulsar_core::theme::Theme) onto a small, semantic palette.
//!
//! Widgets never hardcode colours and never pick a hue per widget: `accent` is the only
//! decorative colour, and `ok`/`warn`/`crit` are reserved for thresholds.

use pulsar_core::theme::Theme;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window background (one colour everywhere).
    pub base: Color,
    /// Slightly lifted background for selected rows / empty bar tracks.
    pub surface: Color,
    /// Hairline borders.
    pub border: Color,
    /// Primary text.
    pub text: Color,
    /// Labels, units, zeros.
    pub dim: Color,
    /// The single accent (titles, active tab).
    pub accent: Color,
    /// Threshold: healthy.
    pub ok: Color,
    /// Threshold: elevated.
    pub warn: Color,
    /// Threshold: critical.
    pub crit: Color,
}

fn rgb(hex: &str) -> Color {
    let (r, g, b) = Theme::parse_hex(hex);
    Color::Rgb(r, g, b)
}

impl Palette {
    pub fn from_theme(t: &Theme) -> Self {
        let base = rgb(&t.bg);
        let text = rgb(&t.fg);
        Palette {
            base,
            surface: blend(base, text, 0.14),
            border: rgb(&t.border),
            text,
            dim: rgb(&t.fg_dim),
            accent: rgb(&t.accent),
            ok: rgb(&t.green),
            warn: rgb(&t.yellow),
            crit: rgb(&t.red),
        }
    }

    /// Value → status colour. `frac` is 0..=1.
    pub fn threshold(&self, frac: f64) -> Color {
        let f = frac.clamp(0.0, 1.0);
        if f < 0.60 {
            self.ok
        } else if f < 0.85 {
            self.warn
        } else {
            self.crit
        }
    }

    /// Category colour for the treemap (not a status).
    pub fn category(&self, i: usize) -> Color {
        match i % 8 {
            0 => self.accent,
            1 => blend(self.accent, self.text, 0.35),
            2 => blend(self.ok, self.accent, 0.35),
            3 => self.ok,
            4 => blend(self.warn, self.accent, 0.35),
            5 => self.warn,
            6 => blend(self.crit, self.accent, 0.30),
            _ => self.crit,
        }
    }
}

/// Linear blend between two colours (`t` in `0..=1`).
pub fn blend(a: Color, b: Color, t: f64) -> Color {
    let (ar, ag, ab) = to_rgb(a);
    let (br, bg, bb) = to_rgb(b);
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        (ar as f64 + (br as f64 - ar as f64) * t) as u8,
        (ag as f64 + (bg as f64 - ag as f64) * t) as u8,
        (ab as f64 + (bb as f64 - ab as f64) * t) as u8,
    )
}

fn to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        _ => (128, 128, 128),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_are_semantic() {
        let p = Palette::from_theme(&Theme::default());
        assert_eq!(p.threshold(0.2), p.ok);
        assert_eq!(p.threshold(0.7), p.warn);
        assert_eq!(p.threshold(0.95), p.crit);
    }

    #[test]
    fn blend_endpoints() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(255, 255, 255);
        assert_eq!(blend(a, b, 0.0), a);
        assert_eq!(blend(a, b, 1.0), b);
    }

    #[test]
    fn surface_is_lifted_from_base() {
        let p = Palette::from_theme(&Theme::default());
        assert_ne!(p.base, p.surface);
    }
}
