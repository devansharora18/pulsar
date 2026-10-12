//! `--dump` mode: render every view headlessly and write text + PNG captures, so Pulsar
//! can produce screenshots without needing a compositor screen-capture API.

use std::path::PathBuf;

use pulsar_core::snapshot::SystemSnapshot;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use tokio::sync::watch;

use crate::state::{AppState, View, ViewMode};
use crate::ui;

const COLS: u16 = 168;
const ROWS: u16 = 46;

pub fn run(dir: PathBuf, png: bool, mut rx: watch::Receiver<SystemSnapshot>) {
    let _ = std::fs::create_dir_all(&dir);
    // Wait for the history to fill (dump mode samples at 100 ms).
    let mut state = AppState::new();
    state.config.refresh_ms = 100;
    for _ in 0..400 {
        if rx.has_changed().unwrap_or(false) {
            state.snapshot = rx.borrow_and_update().clone();
            if state.snapshot.proc_count > 0 && state.snapshot.seq >= 110 {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let themes: Vec<String> = match std::env::var("PULSAR_DUMP_THEMES") {
        Ok(s) => s.split(',').map(|t| t.trim().to_string()).collect(),
        Err(_) => vec![state.config.theme.clone()],
    };

    for (ti, theme_name) in themes.iter().enumerate() {
        state.theme = pulsar_core::theme::Theme::resolve(theme_name);
        state.palette = crate::theme_map::Palette::from_theme(&state.theme);
        for (vi, view) in View::all().iter().enumerate() {
            state.view = *view;
            state.view_mode = ViewMode::Group;
            state.selected = None;
            // Expand the two largest groups so the processes capture shows folding.
            state.expanded.clear();
            if *view == View::Processes {
                for g in state.snapshot.groups.iter().take(2) {
                    state.expanded.insert(g.key.clone());
                }
            }
            let mut terminal = Terminal::new(TestBackend::new(COLS, ROWS)).expect("test terminal");
            terminal.draw(|f| ui::draw(f, &mut state)).expect("draw");
            let buf = terminal.backend().buffer().clone();

            let prefix = if ti == 0 { "01" } else { "02" };
            let base = format!("{prefix}-{}-{}", view_name(*view), slug(theme_name));
            let txt = dir.join(format!("{base}.txt"));
            let _ = std::fs::write(&txt, buffer_to_text(&buf));
            println!("wrote {}", txt.display());
            if png {
                let out = dir.join(format!("{base}.png"));
                match rasterize(&buf, state.palette.bg, &out) {
                    Ok(()) => println!("wrote {}", out.display()),
                    Err(e) => eprintln!("png dump failed: {e}"),
                }
            }
            let _ = vi;
        }
    }
}

fn view_name(v: View) -> &'static str {
    match v {
        View::Overview => "overview",
        View::Processes => "processes",
        View::Memory => "memory",
        View::Power => "power",
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn buffer_to_text(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn rgb(c: Color, fallback_bg: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Reset => match fallback_bg {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        },
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        _ => (128, 128, 128),
    }
}

fn load_font() -> Option<fontdue::Font> {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
    ];
    if let Ok(list) = std::fs::read_dir("/usr/share/fonts") {
        for e in list.flatten() {
            candidates.push(e.path());
        }
    }
    for c in candidates {
        if c.extension().and_then(|e| e.to_str()) == Some("ttf") {
            if let Ok(bytes) = std::fs::read(&c) {
                if let Ok(f) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                    return Some(f);
                }
            }
        }
    }
    None
}

fn rasterize(buf: &Buffer, default_bg: Color, path: &std::path::Path) -> Result<(), String> {
    let font = load_font().ok_or_else(|| "no monospace TTF font found".to_string())?;
    let px = 17.0f32;
    let line = font
        .horizontal_line_metrics(px)
        .ok_or_else(|| "font has no line metrics".to_string())?;
    let ascent = line.ascent;
    let cell_h = line.new_line_size.ceil() as u32;
    let (advance_metrics, _) = font.rasterize('M', px);
    let cell_w = advance_metrics.advance_width.round().max(1.0) as u32;

    let img_w = cell_w * buf.area.width as u32;
    let img_h = cell_h * buf.area.height as u32;
    let mut img = image::RgbImage::new(img_w, img_h);

    // Backgrounds first.
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let Some(cell) = buf.cell((x, y)) else {
                continue;
            };
            let (r, g, b) = rgb(cell.bg, default_bg);
            let color = image::Rgb([r, g, b]);
            for dy in 0..cell_h {
                for dx in 0..cell_w {
                    img.put_pixel(x as u32 * cell_w + dx, y as u32 * cell_h + dy, color);
                }
            }
        }
    }

    // Glyphs.
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let Some(cell) = buf.cell((x, y)) else {
                continue;
            };
            let sym = cell.symbol();
            let Some(ch) = sym.chars().next() else {
                continue;
            };
            if ch == ' ' || ch == '\0' {
                continue;
            }
            let (fg_r, fg_g, fg_b) = rgb(cell.fg, Color::Rgb(230, 230, 230));
            let (metrics, bitmap) = font.rasterize(ch, px);
            let cell_x = (x as u32 * cell_w) as i32;
            let baseline = (y as u32 * cell_h) as i32 + ascent.round() as i32;
            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let a = bitmap[gy * metrics.width + gx];
                    if a == 0 {
                        continue;
                    }
                    let px_x = cell_x + gx as i32 + metrics.xmin;
                    let px_y = baseline - metrics.height as i32 - metrics.ymin + gy as i32;
                    if px_x < 0 || px_y < 0 || px_x >= img_w as i32 || px_y >= img_h as i32 {
                        continue;
                    }
                    let base = img.get_pixel(px_x as u32, px_y as u32).0;
                    let a = a as u16;
                    let ia = 255 - a;
                    let nr = ((fg_r as u16 * a + base[0] as u16 * ia) / 255) as u8;
                    let ng = ((fg_g as u16 * a + base[1] as u16 * ia) / 255) as u8;
                    let nb = ((fg_b as u16 * a + base[2] as u16 * ia) / 255) as u8;
                    img.put_pixel(px_x as u32, px_y as u32, image::Rgb([nr, ng, nb]));
                }
            }
        }
    }

    // Add a little padding frame.
    img.save(path).map_err(|e| e.to_string())
}
