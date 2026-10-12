//! Map the TOML [`Theme`](pulsar_core::theme::Theme) hex colours onto ratatui colours.

use pulsar_core::theme::Theme;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub border: Color,
    pub accent: Color,
    pub green: Color,
    pub yellow: Color,
    pub orange: Color,
    pub red: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
}

fn rgb(hex: &str) -> Color {
    let (r, g, b) = Theme::parse_hex(hex);
    Color::Rgb(r, g, b)
}

impl Palette {
    pub fn from_theme(t: &Theme) -> Self {
        Palette {
            bg: rgb(&t.bg),
            fg: rgb(&t.fg),
            dim: rgb(&t.fg_dim),
            border: rgb(&t.border),
            accent: rgb(&t.accent),
            green: rgb(&t.green),
            yellow: rgb(&t.yellow),
            orange: rgb(&t.orange),
            red: rgb(&t.red),
            blue: rgb(&t.blue),
            magenta: rgb(&t.magenta),
            cyan: rgb(&t.cyan),
        }
    }

    /// Colour for the i-th treemap / list entry.
    pub fn series(&self, i: usize) -> Color {
        const N: usize = 7;
        match i % N {
            0 => self.accent,
            1 => self.blue,
            2 => self.magenta,
            3 => self.cyan,
            4 => self.green,
            5 => self.yellow,
            _ => self.orange,
        }
    }

    /// Green → yellow → red ramp for utilisation `0..=1`, dimmed at low load so an idle
    /// machine reads calm rather than fully saturated.
    pub fn heat(&self, utilization: f64) -> Color {
        let u = utilization.clamp(0.0, 1.0);
        if u < 0.05 {
            self.border
        } else if u < 0.5 {
            blend(self.bg, self.green, 0.35 + 0.5 * (u / 0.5))
        } else if u < 0.8 {
            blend(self.green, self.yellow, (u - 0.5) / 0.3)
        } else {
            blend(self.yellow, self.red, (u - 0.8) / 0.2)
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
