//! Reusable graph primitives: eighth-block bars, sparklines and braille area graphs.
//!
//! All of them are pure and unit-tested; widgets only choose colours and geometry.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme_map::blend;

const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A horizontal bar with sub-cell precision. The filled part uses `color`, the track uses
/// `track`. Returns exactly `width` cells of spans.
pub fn bar_spans(frac: f64, width: usize, color: Color, track: Color) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let f = frac.clamp(0.0, 1.0);
    let total = (f * width as f64 * 8.0).round() as usize;
    let full = total / 8;
    let rem = total % 8;
    let mut filled = String::new();
    filled.push_str(&"█".repeat(full.min(width)));
    if rem > 0 && full < width {
        filled.push(EIGHTHS[rem - 1]);
    }
    let used = full.min(width) + usize::from(rem > 0 && full < width);
    let empty = width.saturating_sub(used);
    vec![
        Span::styled(filled, Style::default().fg(color).bg(track)),
        Span::styled(" ".repeat(empty), Style::default().bg(track)),
    ]
}

/// A 1-row sparkline of the last `width` samples.
pub fn sparkline(values: &[f64], width: usize, max: f64) -> String {
    if width == 0 || values.is_empty() {
        return String::new();
    }
    let start = values.len().saturating_sub(width);
    let slice = &values[start..];
    let max = if max > 0.0 {
        max
    } else {
        slice.iter().copied().fold(0.0_f64, f64::max).max(1e-9)
    };
    slice
        .iter()
        .map(|v| {
            let n = (v / max).clamp(0.0, 1.0);
            let idx = ((n * 7.0).floor() as usize).min(7);
            SPARK[idx]
        })
        .collect()
}

/// A filled braille area graph, one `Line` per terminal row, coloured top→bottom.
pub fn braille_area(
    values: &[f64],
    width: usize,
    height: usize,
    top: Color,
    bottom: Color,
) -> Vec<Line<'static>> {
    let max = values.iter().copied().fold(0.0_f64, f64::max);
    braille_area_max(values, width, height, top, bottom, max)
}

/// Same as [`braille_area`] but with an explicit ceiling, so a flat series does not fill the
/// whole panel.
pub fn braille_area_max(
    values: &[f64],
    width: usize,
    height: usize,
    top: Color,
    bottom: Color,
    max: f64,
) -> Vec<Line<'static>> {
    if width == 0 || height == 0 || values.is_empty() {
        return Vec::new();
    }
    let dots_w = width * 2;
    let dots_h = height * 4;
    let max = max.max(1e-9);

    // Sample the series into dots_w columns.
    let sampled: Vec<f64> = (0..dots_w)
        .map(|i| {
            if values.len() == 1 {
                values[0]
            } else {
                let pos = i as f64 * (values.len() - 1) as f64 / (dots_w - 1) as f64;
                let lo = pos.floor() as usize;
                let hi = (lo + 1).min(values.len() - 1);
                let t = pos - lo as f64;
                values[lo] * (1.0 - t) + values[hi] * t
            }
        })
        .collect();

    let mut grid = vec![vec![false; dots_w]; dots_h];
    for (x, v) in sampled.iter().enumerate() {
        let n = (v / max).clamp(0.0, 1.0);
        let y = ((1.0 - n) * (dots_h - 1) as f64).round() as usize;
        for row in grid.iter_mut().skip(y) {
            row[x] = true;
        }
    }

    // Braille dot masks: [dy][dx].
    let masks = [[0x01_u32, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

    let mut lines = Vec::with_capacity(height);
    for cy in 0..height {
        let mut row = String::with_capacity(width);
        let t = if height > 1 {
            cy as f64 / (height - 1) as f64
        } else {
            0.0
        };
        let color = blend(top, bottom, t);
        for cx in 0..width {
            let mut bits = 0u32;
            for (dy, mask) in masks.iter().enumerate() {
                for (dx, bit) in mask.iter().enumerate() {
                    if grid[cy * 4 + dy][cx * 2 + dx] {
                        bits |= bit;
                    }
                }
            }
            row.push(char::from_u32(0x2800 + bits).unwrap_or(' '));
        }
        lines.push(Line::from(Span::styled(row, Style::default().fg(color))));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width_of(spans: &[Span]) -> usize {
        spans.iter().map(|s| s.content.chars().count()).sum()
    }

    #[test]
    fn bar_is_exact_width() {
        let s = bar_spans(0.5, 10, Color::Red, Color::Black);
        assert_eq!(width_of(&s), 10);
        assert!(s[0].content.contains('█'));
    }

    #[test]
    fn bar_subcell_precision() {
        // 1 cell, 1/8 filled → ▏ (not empty, not full)
        let s = bar_spans(1.0 / 8.0, 1, Color::Red, Color::Black);
        assert_eq!(s[0].content, "▏");
        // 1 cell, 100% → █
        let s = bar_spans(1.0, 1, Color::Red, Color::Black);
        assert_eq!(s[0].content, "█");
        // zero → track only
        let s = bar_spans(0.0, 3, Color::Red, Color::Black);
        assert_eq!(width_of(&s), 3);
        assert!(s[0].content.is_empty());
    }

    #[test]
    fn sparkline_levels() {
        assert_eq!(sparkline(&[0.0, 100.0], 2, 100.0), "▁█");
        assert_eq!(sparkline(&[50.0], 1, 100.0), "▄");
        assert_eq!(sparkline(&[], 3, 1.0), "");
    }

    #[test]
    fn braille_rows_have_requested_geometry() {
        let data: Vec<f64> = (0..50).map(|i| (i as f64 / 50.0) * 10.0).collect();
        let rows = braille_area(&data, 20, 3, Color::Green, Color::Blue);
        assert_eq!(rows.len(), 3);
        // Every line is exactly 20 terminal cells wide.
        for line in &rows {
            let n: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            assert_eq!(n, 20);
        }
        // Non-empty braille glyphs.
        let text: String = rows
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
            .collect();
        assert!(text.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)));
    }

    #[test]
    fn braille_degenerate_inputs() {
        assert!(braille_area(&[], 10, 2, Color::Green, Color::Blue).is_empty());
        assert!(braille_area(&[1.0], 0, 2, Color::Green, Color::Blue).is_empty());
    }
}
