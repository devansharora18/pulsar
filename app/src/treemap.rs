//! Squarified treemap layout (Bruls, Huizing, van Wijk). Pure geometry so it can be tested
//! and reused by the interactive memory view.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
}

/// Lay `values` out in a `w × h` box using the squarified algorithm. Returns one rect per
/// input value, in the same order. Zero/negative values get a zero-area rect.
pub fn squarify(values: &[f64], w: f64, h: f64) -> Vec<Rect> {
    let mut out = vec![Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; values.len()];
    let total: f64 = values.iter().filter(|v| **v > 0.0).sum();
    if total <= 0.0 || w <= 0.0 || h <= 0.0 {
        return out;
    }
    // Work on indices sorted descending by value.
    let mut idx: Vec<usize> = (0..values.len()).filter(|i| values[*i] > 0.0).collect();
    idx.sort_by(|a, b| values[*b].partial_cmp(&values[*a]).unwrap_or(std::cmp::Ordering::Equal));

    let scale = (w * h) / total;
    let areas: Vec<f64> = idx.iter().map(|i| values[*i] * scale).collect();

    let mut x = 0.0;
    let mut y = 0.0;
    let mut cw = w;
    let mut ch = h;
    let mut i = 0;
    while i < idx.len() {
        let short = cw.min(ch);
        // Grow the current row while the aspect ratio improves.
        let mut row_area = 0.0;
        let mut best = f64::INFINITY;
        let mut end = i;
        while end < idx.len() {
            row_area += areas[end];
            let side = row_area / short;
            let worst = worst_ratio(&areas[i..=end], side);
            if worst <= best {
                best = worst;
                end += 1;
            } else {
                break;
            }
        }
        if end == i {
            end = i + 1;
        }
        // Emit the row [i, end).
        let row_area: f64 = areas[i..end].iter().sum();
        let side = (row_area / short).max(0.0);
        if cw >= ch {
            // Vertical column on the left, width = side.
            let col_w = side.min(cw);
            let mut cy = y;
            for k in i..end {
                let hh = if row_area > 0.0 { areas[k] / row_area * ch } else { 0.0 };
                out[idx[k]] = Rect { x, y: cy, w: col_w, h: hh };
                cy += hh;
            }
            x += col_w;
            cw -= col_w;
        } else {
            // Horizontal row on top, height = side.
            let row_h = side.min(ch);
            let mut cx = x;
            for k in i..end {
                let ww = if row_area > 0.0 { areas[k] / row_area * cw } else { 0.0 };
                out[idx[k]] = Rect { x: cx, y, w: ww, h: row_h };
                cx += ww;
            }
            y += row_h;
            ch -= row_h;
        }
        i = end;
    }
    out
}

fn worst_ratio(row: &[f64], side: f64) -> f64 {
    if side <= 0.0 {
        return f64::INFINITY;
    }
    let sum: f64 = row.iter().sum();
    let side2 = side * side;
    let sum2 = sum * sum;
    let mut worst: f64 = 0.0;
    for a in row {
        if *a <= 0.0 {
            continue;
        }
        let r = (side2 * a / sum2).max(sum2 / (side2 * a));
        worst = worst.max(r);
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_are_proportional() {
        let vals = [50.0, 25.0, 25.0];
        let rects = squarify(&vals, 100.0, 100.0);
        let total: f64 = rects.iter().map(|r| r.area()).sum();
        assert!((total - 100.0 * 100.0).abs() < 1.0, "total area {total}");
        // Largest should be roughly half the area.
        assert!((rects[0].area() / (100.0 * 100.0) - 0.5).abs() < 0.05);
    }

    #[test]
    fn degenerate_inputs_are_safe() {
        assert_eq!(squarify(&[], 100.0, 100.0).len(), 0);
        let r = squarify(&[0.0, 0.0], 100.0, 100.0);
        assert!(r.iter().all(|x| x.area() == 0.0));
        let r = squarify(&[10.0], 0.0, 100.0);
        assert_eq!(r[0].area(), 0.0);
    }

    #[test]
    fn single_value_fills_box() {
        let r = squarify(&[1.0], 80.0, 40.0);
        assert!((r[0].area() - 80.0 * 40.0).abs() < 1.0);
    }
}
