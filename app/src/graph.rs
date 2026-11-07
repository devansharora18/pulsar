//! Reusable graph primitives: bars, sparklines and a filled braille area graph.
//!
//! All of them are pure (or draw into a `Buffer`) and unit-tested; widgets only choose
//! colours and geometry.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::theme_map::blend;

const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A horizontal bar with sub-cell precision. Full cells are `█` with foreground *and*
/// background set to the bar colour, so there are no hairline seams between cells; the
/// tail is an eighth-block glyph, and the track is `track`.
pub fn bar_spans(frac: f64, width: usize, color: Color, track: Color) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let f = frac.clamp(0.0, 1.0);
    let total = (f * width as f64 * 8.0).round() as usize;
    let full = (total / 8).min(width);
    let rem = total % 8;
    let mut spans = Vec::new();
    if full > 0 {
        spans.push(Span::styled(
            "█".repeat(full),
            Style::default().fg(color).bg(color),
        ));
    }
    if rem > 0 && full < width {
        spans.push(Span::styled(
            EIGHTHS[rem - 1].to_string(),
            Style::default().fg(color).bg(track),
        ));
    }
    let used = full + usize::from(rem > 0 && full < width);
    let empty = width.saturating_sub(used);
    if empty > 0 {
        spans.push(Span::styled(" ".repeat(empty), Style::default().bg(track)));
    }
    spans
}

/// Paint a bar directly into a buffer (used by the per-core tiles).
pub fn paint_bar(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: usize,
    frac: f64,
    color: Color,
    track: Color,
) {
    if width == 0 {
        return;
    }
    let total = (frac.clamp(0.0, 1.0) * width as f64 * 8.0).round() as usize;
    let full = (total / 8).min(width);
    let rem = total % 8;
    for i in 0..width {
        if let Some(cell) = buf.cell_mut((x + i as u16, y)) {
            if i < full {
                cell.set_bg(color);
                cell.set_fg(color);
                cell.set_symbol(" ");
            } else if i == full && rem > 0 {
                cell.set_bg(track);
                cell.set_fg(color);
                cell.set_symbol(&EIGHTHS[rem - 1].to_string());
            } else {
                cell.set_bg(track);
                cell.set_symbol(" ");
            }
        }
    }
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

/// Braille dot masks, indexed by [row within the cell][column within the cell].
fn mask(dy: usize, dx: usize) -> u32 {
    const MASKS: [[u32; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
    MASKS[dy.min(3)][dx.min(1)]
}

fn braille_char(bits: u32) -> String {
    char::from_u32(0x2800 + bits).unwrap_or(' ').to_string()
}

/// Draw a filled area graph with a braille line along the top edge and a vertical gradient.
///
/// `from_top = false` grows upward from the bottom of `area` (network received / power);
/// `from_top = true` grows downward from the top (network sent). The fill is a solid
/// background gradient (no dotted texture) and the boundary is drawn with braille dots.
#[allow(clippy::too_many_arguments)]
pub fn draw_area(
    buf: &mut Buffer,
    area: Rect,
    values: &[f64],
    top: Color,
    bottom: Color,
    line: Color,
    max: f64,
    from_top: bool,
) {
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 || values.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let max = max.max(1e-9);
    let dots_h = h * 4;
    let dots_w = w * 2;

    // Number of filled dots from the baseline for each braille sub-column.
    let mut filled = vec![0usize; dots_w];
    for (sx, slot) in filled.iter_mut().enumerate() {
        let v = sample(values, sx, dots_w);
        let n = (v / max).clamp(0.0, 1.0);
        *slot = (n * dots_h as f64).round() as usize;
    }

    for cy in 0..h {
        let t = if h > 1 {
            cy as f64 / (h - 1) as f64
        } else {
            0.0
        };
        let fill_color = blend(top, bottom, t);
        for cx in 0..w {
            let x = area.x + cx as u16;
            let y = area.y + cy as u16;
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            let mut bits = 0u32;
            let mut has_fill = false;
            for sub in 0..2 {
                let f = filled[cx * 2 + sub];
                if from_top {
                    // Fill dots 0..f; the line sits at dot f.
                    if f > cy * 4 {
                        has_fill = true;
                    }
                    if f >= cy * 4 && f <= cy * 4 + 3 {
                        bits |= mask(f - cy * 4, sub);
                    }
                } else {
                    // Fill dots b..dots_h; the line sits at dot b.
                    let b = dots_h.saturating_sub(f);
                    if b <= cy * 4 + 3 {
                        has_fill = true;
                    }
                    if b >= cy * 4 && b <= cy * 4 + 3 {
                        bits |= mask(b - cy * 4, sub);
                    }
                }
            }
            if has_fill {
                cell.set_bg(fill_color);
                if bits != 0 {
                    cell.set_fg(line);
                    cell.set_symbol(&braille_char(bits));
                } else {
                    cell.set_symbol(" ");
                }
            }
        }
    }
}

fn sample(values: &[f64], i: usize, n: usize) -> f64 {
    if values.len() == 1 || n <= 1 {
        return values[0];
    }
    let pos = i as f64 * (values.len() - 1) as f64 / (n - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(values.len() - 1);
    let t = pos - lo as f64;
    values[lo] * (1.0 - t) + values[hi] * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width_of(spans: &[Span]) -> usize {
        spans.iter().map(|s| s.content.chars().count()).sum()
    }

    #[test]
    fn bar_is_exact_width_and_seamless() {
        let s = bar_spans(0.5, 10, Color::Red, Color::Black);
        assert_eq!(width_of(&s), 10);
        // Full cells are solid foreground+background runs.
        assert_eq!(s[0].content, "█████");
        assert_eq!(s[0].style.fg, Some(Color::Red));
        assert_eq!(s[0].style.bg, Some(Color::Red));
    }

    #[test]
    fn bar_subcell_precision() {
        let s = bar_spans(1.0 / 8.0, 1, Color::Red, Color::Black);
        assert_eq!(s[0].content, "▏");
        let s = bar_spans(1.0, 1, Color::Red, Color::Black);
        assert_eq!(s[0].content, "█");
        let s = bar_spans(0.0, 3, Color::Red, Color::Black);
        assert_eq!(width_of(&s), 3);
        assert_eq!(s[0].content, "   ");
    }

    #[test]
    fn sparkline_levels() {
        assert_eq!(sparkline(&[0.0, 100.0], 2, 100.0), "▁█");
        assert_eq!(sparkline(&[50.0], 1, 100.0), "▄");
        assert_eq!(sparkline(&[], 3, 1.0), "");
    }

    #[test]
    fn area_fills_and_draws_a_line() {
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        let data = vec![10.0; 20];
        draw_area(
            &mut buf,
            area,
            &data,
            Color::Blue,
            Color::Black,
            Color::White,
            10.0,
            false,
        );
        // A full-scale series fills every cell.
        let filled = (0..10)
            .flat_map(|x| (0..4).map(move |y| (x, y)))
            .filter(|(x, y)| buf.cell((*x as u16, *y as u16)).unwrap().bg != Color::Reset)
            .count();
        assert_eq!(filled, 40);
    }

    #[test]
    fn area_empty_series_draws_nothing() {
        let area = Rect::new(0, 0, 8, 3);
        let mut buf = Buffer::empty(area);
        draw_area(
            &mut buf,
            area,
            &[0.0; 8],
            Color::Blue,
            Color::Black,
            Color::White,
            10.0,
            false,
        );
        let filled = (0..8)
            .flat_map(|x| (0..3).map(move |y| (x, y)))
            .filter(|(x, y)| buf.cell((*x as u16, *y as u16)).unwrap().bg != Color::Reset)
            .count();
        assert_eq!(filled, 0);
    }

    #[test]
    fn area_degenerate_inputs() {
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        draw_area(
            &mut buf,
            area,
            &[],
            Color::Blue,
            Color::Black,
            Color::White,
            1.0,
            false,
        );
    }
}
