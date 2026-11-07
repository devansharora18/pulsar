//! All TUI rendering: header, tabs, the four views and the shared widgets.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Table, TableState, Tabs, Wrap,
};
use ratatui::Frame;

use pulsar_core::group::{Group, ProcSample};

use crate::format::{human_bytes, human_kb, human_rate, human_temp, human_watts, pct};
use crate::graph::{bar_spans, braille_area};
use crate::state::{AppState, Overlay, View, ViewMode};
use crate::theme_map::blend;
use crate::treemap::squarify;

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let area = f.area();
    // One background everywhere: no black patches between widgets.
    f.render_widget(
        Block::default().style(Style::default().bg(state.palette.base)),
        area,
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // tabs
            Constraint::Min(6),    // body
            Constraint::Length(1), // footer
        ])
        .split(area);

    draw_header(f, state, chunks[0]);
    draw_tabs(f, state, chunks[1]);
    match state.view {
        View::Overview => draw_overview(f, state, chunks[2]),
        View::Processes => draw_processes(f, state, chunks[2]),
        View::Memory => draw_memory(f, state, chunks[2]),
        View::Power => draw_power(f, state, chunks[2]),
        View::Ports => draw_ports(f, state, chunks[2]),
    }
    draw_footer(f, state, chunks[3]);

    crate::overlays::draw(f, state, area);
}

// ── chrome ───────────────────────────────────────────────────────────────────

fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let mem = &s.mem;
    let power = s.power;
    let sep = || Span::styled(" │ ", Style::default().fg(p.border));

    let mut spans = vec![
        Span::styled(
            " PULSAR ",
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ),
        sep(),
        Span::styled("cpu ", Style::default().fg(p.dim)),
        Span::styled(
            format!("{:.1}", s.cpu.overall_usage_pct),
            Style::default().fg(p.text),
        ),
        Span::styled("%", Style::default().fg(p.dim)),
        sep(),
        Span::styled("mem ", Style::default().fg(p.dim)),
        Span::styled(
            format!("{:.1}", mem.used_real as f64 / 1024.0 / 1024.0),
            Style::default().fg(p.text),
        ),
        Span::styled(" GB", Style::default().fg(p.dim)),
        sep(),
        Span::styled(
            if state.config.nerd_icons {
                "⚡ "
            } else {
                "pwr "
            },
            Style::default().fg(p.dim),
        ),
        Span::styled(format!("{:.1}", power.total_w), Style::default().fg(p.text)),
        Span::styled(" W", Style::default().fg(p.dim)),
        sep(),
        Span::styled(
            format!("{} procs", s.proc_count),
            Style::default().fg(p.dim),
        ),
    ];

    // Hardware name only when there is room.
    if area.width >= 110 {
        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let model = format!("{} ", s.cpu.model);
        let room = area.width as usize;
        if used + model.chars().count() + 2 < room {
            spans.push(Span::styled(
                " ".repeat(room - used - model.chars().count() - 1),
                Style::default(),
            ));
            spans.push(Span::styled(model, Style::default().fg(p.dim)));
        }
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(p.base)),
        area,
    );
}

fn draw_tabs(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    // Icons are a nicety; drop them on narrow terminals so all tabs fit.
    let show_icons = state.config.nerd_icons && area.width >= 110;
    let mut titles: Vec<Line> = View::all()
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let label = if show_icons {
                format!("{} {}", v.icon(), v.label())
            } else {
                v.label().to_string()
            };
            Line::from(vec![
                Span::styled(format!(" {} ", i + 1), Style::default().fg(p.dim)),
                Span::styled(label, Style::default().fg(p.dim)),
                Span::styled(" ", Style::default()),
            ])
        })
        .collect();
    titles.push(Line::from(vec![
        Span::styled(" 6 ", Style::default().fg(p.dim)),
        Span::styled(
            if show_icons {
                "⚙ Settings"
            } else {
                "Settings"
            },
            Style::default().fg(p.dim),
        ),
        Span::styled(" ", Style::default()),
    ]));

    let selected = if state.overlay == Some(Overlay::Settings) {
        View::all().len()
    } else {
        View::all()
            .iter()
            .position(|v| *v == state.view)
            .unwrap_or(0)
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(p.dim).bg(p.base))
        .highlight_style(
            Style::default()
                .fg(p.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(Span::styled("│", Style::default().fg(p.border)));
    f.render_widget(tabs, area);
}

fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let line = if let Some(msg) = state.current_message() {
        Line::from(Span::styled(format!(" {msg}"), Style::default().fg(p.warn)))
    } else {
        let key = Style::default().fg(p.text);
        let act = Style::default().fg(p.dim);
        let compact = area.width < 100;
        let mut spans = vec![
            Span::styled("1-5", key),
            Span::styled(" views  ", act),
            Span::styled("/", key),
            Span::styled(" search  ", act),
            Span::styled("j/k", key),
            Span::styled(" move  ", act),
        ];
        if !compact {
            spans.push(Span::styled("enter", key));
            spans.push(Span::styled(" expand  ", act));
        }
        spans.push(Span::styled("x", key));
        spans.push(Span::styled(" kill  ", act));
        spans.push(Span::styled("s / 6", key));
        spans.push(Span::styled(" settings  ", act));
        spans.push(Span::styled("?", key));
        spans.push(Span::styled(" help  ", act));
        spans.push(Span::styled("q", key));
        spans.push(Span::styled(" quit", act));
        Line::from(spans)
    };
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(p.base)),
        area,
    );
}

fn panel(state: &AppState, title: &str) -> Block<'static> {
    let p = state.palette;
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.border))
        .padding(Padding::new(1, 1, 0, 0))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(p.base))
}

// ── overview ─────────────────────────────────────────────────────────────────

fn draw_overview(f: &mut Frame, state: &AppState, area: Rect) {
    let compact = area.height < 30;
    let cores = state.snapshot.cpu.cores.len();
    let core_cols = 4u16;
    let core_rows = (cores as u16).div_ceil(core_cols).max(1);
    let cores_h = core_rows * if compact { 2 } else { 3 } + 2;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(cores_h),
            Constraint::Min(4),
        ])
        .split(cols[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(if compact { 6 } else { 9 }),
            Constraint::Length(if compact { 8 } else { 10 }),
        ])
        .split(cols[1]);

    draw_resources(f, state, left[0]);
    draw_cores(f, state, left[1], compact);
    draw_sensors(f, state, left[2]);
    draw_top_mem(f, state, right[0]);
    draw_network(f, state, right[1]);
    draw_disk(f, state, right[2]);
}

/// Split "18.4 GB" / "12.3%" into a bright value and a dim unit.
fn val_spans(s: impl Into<String>, p: &crate::theme_map::Palette) -> Vec<Span<'static>> {
    let s = s.into();
    if let Some((v, u)) = s.rsplit_once(' ') {
        if !u.is_empty() {
            return vec![
                Span::styled(v.to_string(), Style::default().fg(p.text)),
                Span::styled(format!(" {u}"), Style::default().fg(p.dim)),
            ];
        }
    }
    if let Some(v) = s.strip_suffix('%') {
        return vec![
            Span::styled(v.to_string(), Style::default().fg(p.text)),
            Span::styled("%", Style::default().fg(p.dim)),
        ];
    }
    vec![Span::styled(s, Style::default().fg(p.text))]
}

fn draw_resources(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let mem = &s.mem;
    let block = panel(state, "Resources");
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 || inner.width < 10 {
        return;
    }
    let label_w = 5u16;
    let value_w = 10u16;
    let bar_w = inner.width.saturating_sub(label_w + value_w + 2) as usize;

    let pmax = state.config.tdp_w.map(|t| t * 2.0).unwrap_or(65.0);
    let items = [
        (
            "CPU",
            state.cpu_ema / 100.0,
            format!("{:.1}%", state.cpu_ema),
        ),
        (
            "MEM",
            if mem.total > 0 {
                mem.used_real as f64 / mem.total as f64
            } else {
                0.0
            },
            human_kb(mem.used_real),
        ),
        (
            "SWAP",
            if mem.swap_total > 0 {
                mem.swap_used as f64 / mem.swap_total as f64
            } else {
                0.0
            },
            human_kb(mem.swap_used),
        ),
        (
            "PWR",
            (state.power_ema / pmax).clamp(0.0, 1.0),
            human_watts(state.power_ema),
        ),
    ];
    for (i, (label, frac, value)) in items.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let mut spans = vec![
            Span::styled(format!("{label:<5}"), Style::default().fg(p.dim)),
            Span::styled(" ", Style::default()),
        ];
        spans.extend(bar_spans(*frac, bar_w, p.threshold(*frac), p.surface));
        spans.push(Span::styled(" ", Style::default()));
        spans.extend(val_spans(
            format!("{value:>width$}", width = value_w as usize - 1),
            &p,
        ));
        f.render_widget(
            Paragraph::new(Line::from(spans)).style(Style::default().bg(p.base)),
            Rect {
                x: inner.x,
                y: inner.y + i as u16,
                width: inner.width,
                height: 1,
            },
        );
    }
}

fn draw_cores(f: &mut Frame, state: &AppState, area: Rect, compact: bool) {
    let p = state.palette;
    let block = panel(state, "CPU cores");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let cores = &state.snapshot.cpu.cores;
    if cores.is_empty() || inner.width < 8 || inner.height < 2 {
        return;
    }
    let cols = 4u16;
    let cell_w = (inner.width / cols).max(4);
    let cell_h = if compact { 2 } else { 3 };
    let buf = f.buffer_mut();
    let text_w = cell_w.saturating_sub(1) as usize;
    for (i, _c) in cores.iter().enumerate() {
        let ema = state.core_ema.get(i).copied().unwrap_or(0.0);
        let x = inner.x + (i as u16 % cols) * cell_w;
        let y = inner.y + (i as u16 / cols) * cell_h;
        if y + 1 >= inner.bottom() {
            break;
        }
        let color = p.threshold(ema / 100.0);
        let usage = format!("{ema:>3.0}%");
        buf.set_stringn(x, y, &usage, text_w, Style::default().fg(color));
        let spark = state
            .core_hist
            .get(i)
            .map(|h| crate::graph::sparkline(h, text_w, 100.0))
            .unwrap_or_default();
        if !spark.is_empty() {
            buf.set_stringn(x, y + 1, &spark, text_w, Style::default().fg(color));
        }
    }
}

fn draw_sensors(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let block = panel(state, "Load & sensors");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let s = &state.snapshot;
    let mut lines = vec![Line::from(vec![
        Span::styled("load ", Style::default().fg(p.dim)),
        Span::styled(
            format!("{:.2} {:.2} {:.2}", s.cpu.load1, s.cpu.load5, s.cpu.load15),
            Style::default().fg(p.text),
        ),
    ])];
    for t in s.temps.iter().take(inner.height.saturating_sub(1) as usize) {
        let color = if t.temp_c >= 80.0 {
            p.crit
        } else if t.temp_c >= 65.0 {
            p.warn
        } else {
            p.ok
        };
        let name = format!("{} {}", t.chip, t.label);
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<22}", trunc(&name, 22)),
                Style::default().fg(p.dim),
            ),
            Span::styled(human_temp(t.temp_c), Style::default().fg(color)),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_top_mem(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let block = panel(state, "Top memory (PSS)");
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let mut groups: Vec<&Group> = state.snapshot.groups.iter().filter(|g| g.pss > 0).collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.pss));
    let max = groups.first().map(|g| g.pss).unwrap_or(1).max(1);
    let name_w = 24u16.min(inner.width.saturating_sub(12));
    let value_w = 10u16;
    let bar_w = inner.width.saturating_sub(name_w + value_w + 2) as usize;
    let mut lines = Vec::new();
    for g in groups.iter().take(inner.height as usize) {
        let frac = g.pss as f64 / max as f64;
        let mut spans = vec![
            Span::styled(
                format!(
                    "{:<width$}",
                    trunc(&g.name, name_w as usize),
                    width = name_w as usize
                ),
                Style::default().fg(p.text),
            ),
            Span::styled(" ", Style::default()),
        ];
        spans.extend(bar_spans(frac, bar_w, p.accent, p.surface));
        spans.push(Span::styled(" ", Style::default()));
        spans.extend(val_spans(
            format!(
                "{:>width$}",
                human_bytes(g.pss as f64 * 1024.0),
                width = value_w as usize - 1
            ),
            &p,
        ));
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_network(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let block = panel(state, "Network");
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 {
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    let mut line = Vec::new();
    for iface in s.net.iter().filter(|i| i.up || i.rx_bps > 0.0).take(2) {
        line.push(Span::styled(
            format!("{} ", trunc(&iface.name, 12)),
            Style::default().fg(p.dim),
        ));
        line.push(Span::styled(
            format!("↓{:<11}", human_rate(iface.rx_bps)),
            Style::default().fg(p.text),
        ));
        line.push(Span::styled(
            format!("↑{}  ", human_rate(iface.tx_bps)),
            Style::default().fg(p.dim),
        ));
    }
    if line.is_empty() {
        line.push(Span::styled(
            "no active interface",
            Style::default().fg(p.dim),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(line)), rows[0]);

    let h = rows[1].height as usize;
    let w = rows[1].width as usize;
    let active = s.series.net_rx.iter().any(|v| *v > 0.5);
    if h > 0 && w > 2 && active {
        let nmax = s
            .series
            .net_rx
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
            .max(1024.0)
            * 1.2;
        let graph = crate::graph::braille_area_max(
            &s.series.net_rx,
            w,
            h,
            p.accent,
            blend(p.accent, p.base, 0.90),
            nmax,
        );
        f.render_widget(Paragraph::new(graph), rows[1]);
    } else if h > 0 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("idle", Style::default().fg(p.dim)))),
            rows[1],
        );
    }
}

fn draw_disk(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let block = panel(state, "Disk");
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let graph_h = if inner.height >= 8 { 4 } else { 0 } as u16;
    let list_h = inner
        .height
        .saturating_sub(graph_h + u16::from(graph_h > 0));
    let active = s
        .disks
        .iter()
        .any(|d| d.read_bps > 0.0 || d.write_bps > 0.0)
        || s.series.disk_read.iter().any(|v| *v > 0.5);
    if !active {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("idle", Style::default().fg(p.dim)))),
            inner,
        );
        return;
    }
    let mut disks: Vec<&pulsar_core::disk::DiskStat> = s
        .disks
        .iter()
        .filter(|d| d.read_bps > 0.0 || d.write_bps > 0.0)
        .collect();
    disks.sort_by_key(|d| std::cmp::Reverse(d.read_bps as u64 + d.write_bps as u64));

    let mut lines = Vec::new();
    for d in disks.iter().take(list_h as usize) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<12}", trunc(&d.name, 12)),
                Style::default().fg(p.text),
            ),
            Span::styled(
                format!("r {:>9}", human_rate(d.read_bps)),
                Style::default().fg(p.dim),
            ),
            Span::styled(
                format!("  w {:>9}", human_rate(d.write_bps)),
                Style::default().fg(p.dim),
            ),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "idle now",
            Style::default().fg(p.dim),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);

    if graph_h > 0 && inner.width > 2 {
        let graph_area = Rect {
            x: inner.x,
            y: inner.y + list_h + 1,
            width: inner.width,
            height: graph_h,
        };
        let graph = braille_area(
            &s.series.disk_read,
            graph_area.width as usize,
            graph_area.height as usize,
            p.accent,
            blend(p.accent, p.base, 0.90),
        );
        f.render_widget(Paragraph::new(graph), graph_area);
    }
}

// ── processes ────────────────────────────────────────────────────────────────

fn draw_processes(f: &mut Frame, state: &mut AppState, area: Rect) {
    let p = state.palette;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // toolbar
            Constraint::Min(6),    // table
            Constraint::Length(2), // selection/action bar
        ])
        .split(area);

    // Toolbar.
    let mode = state.view_mode.label();
    let mut tool = vec![
        Span::styled(" view:", Style::default().fg(p.dim)),
        Span::styled(mode, Style::default().fg(p.accent)),
        Span::styled("  sort:", Style::default().fg(p.dim)),
        Span::styled(
            format!(
                "{} {}",
                state.sort.label(),
                if state.sort_desc { "▼" } else { "▲" }
            ),
            Style::default().fg(p.accent),
        ),
        Span::styled("  groups:", Style::default().fg(p.dim)),
        Span::styled(
            format!("{}", state.snapshot.groups.len()),
            Style::default().fg(p.text),
        ),
    ];
    if state.hide_kernel {
        tool.push(Span::styled(
            "  [kernel hidden]",
            Style::default().fg(p.warn),
        ));
    }
    if state.search_active || !state.search.is_empty() {
        tool.push(Span::styled("  /", Style::default().fg(p.accent)));
        tool.push(Span::styled(
            if state.search.is_empty() {
                "type…".into()
            } else {
                state.search.clone()
            },
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(tool)), chunks[0]);

    let selected_idx = draw_process_table(f, state, chunks[1]);
    draw_action_bar(f, state, chunks[2], selected_idx);
}

fn cpu_cell(value: f64, p: &crate::theme_map::Palette) -> Cell<'static> {
    if value < 0.05 {
        return Cell::from(
            Line::from(Span::styled("·", Style::default().fg(p.dim))).alignment(Alignment::Right),
        );
    }
    let frac = (value / 100.0).clamp(0.0, 1.0);
    let mut spans = vec![
        Span::styled(format!("{value:>3.0}%"), Style::default().fg(p.text)),
        Span::styled(" ", Style::default()),
    ];
    spans.extend(bar_spans(frac, 5, p.threshold(frac), p.base));
    Cell::from(Line::from(spans).alignment(Alignment::Right))
}

fn mem_cell(kb: u64, max_kb: u64, p: &crate::theme_map::Palette) -> Cell<'static> {
    if kb == 0 {
        return Cell::from(
            Line::from(Span::styled("·", Style::default().fg(p.dim))).alignment(Alignment::Right),
        );
    }
    let frac = if max_kb > 0 {
        (kb as f64 / max_kb as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut spans = val_spans(format!("{:>6}", human_bytes(kb as f64 * 1024.0)), p);
    spans.push(Span::styled(" ", Style::default()));
    spans.extend(bar_spans(frac, 5, p.accent, p.base));
    Cell::from(Line::from(spans).alignment(Alignment::Right))
}

fn io_cell(bytes: u64, p: &crate::theme_map::Palette) -> Cell<'static> {
    if bytes == 0 {
        return Cell::from(
            Line::from(Span::styled("·", Style::default().fg(p.dim))).alignment(Alignment::Right),
        );
    }
    Cell::from(
        Line::from(Span::styled(
            human_bytes(bytes as f64),
            Style::default().fg(p.dim),
        ))
        .alignment(Alignment::Right),
    )
}

fn power_cell(watts: f64, p: &crate::theme_map::Palette) -> Cell<'static> {
    if watts < 0.0005 {
        return Cell::from(
            Line::from(Span::styled("·", Style::default().fg(p.dim))).alignment(Alignment::Right),
        );
    }
    Cell::from(Line::from(val_spans(human_watts(watts), p)).alignment(Alignment::Right))
}

fn draw_process_table(f: &mut Frame, state: &mut AppState, area: Rect) -> Option<usize> {
    let p = state.palette;
    let (visible, child_map, max_pss) = {
        let keys = state.visible_keys();
        let snap = state.snapshot.clone();
        let mut groups: Vec<Group> = keys
            .iter()
            .filter_map(|k| snap.groups.iter().find(|g| &g.key == k).cloned())
            .collect();
        groups.sort_by_key(|g| !state.is_pinned(&g.key));
        let max_pss = groups.iter().map(|g| g.pss).max().unwrap_or(1).max(1);
        let mut child_map: std::collections::HashMap<String, Vec<ProcSample>> =
            std::collections::HashMap::new();
        for g in &groups {
            let kids: Vec<ProcSample> = g
                .pids
                .iter()
                .filter_map(|pid| snap.process_by_pid(*pid).cloned())
                .collect();
            child_map.insert(g.key.clone(), kids);
        }
        (groups, child_map, max_pss)
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut selected_row: Option<usize> = None;
    for g in &visible {
        let is_sel = state.selected.as_deref() == Some(g.key.as_str());
        if is_sel {
            selected_row = Some(rows.len());
        }
        let pinned = state.is_pinned(&g.key);
        let name = format!("{}{}", if pinned { "★ " } else { "" }, trunc(&g.name, 30));
        let mut row = Row::new(vec![
            Cell::from(Line::from(name).style(Style::default().fg(p.text))),
            Cell::from(
                Line::from(Span::styled(
                    if g.count() > 1 {
                        g.count().to_string()
                    } else {
                        String::new()
                    },
                    Style::default().fg(p.dim),
                ))
                .alignment(Alignment::Right),
            ),
            cpu_cell(g.cpu_pct, &p),
            mem_cell(g.pss, max_pss, &p),
            io_cell(g.io_read + g.io_write, &p),
            power_cell(g.power_w, &p),
            Cell::from(
                Line::from(Span::styled(
                    g.pids
                        .first()
                        .map(|pid| pid.to_string())
                        .unwrap_or_default(),
                    Style::default().fg(p.dim),
                ))
                .alignment(Alignment::Right),
            ),
        ]);
        if is_sel {
            row = row.style(Style::default().bg(p.surface));
        }
        rows.push(row);

        let expanded = state.view_mode == ViewMode::Tree || state.expanded.contains(&g.key);
        if expanded {
            if let Some(kids) = child_map.get(&g.key) {
                let n = kids.len();
                for (j, kid) in kids.iter().enumerate() {
                    let guide = if j + 1 == n { "└─ " } else { "├─ " };
                    let mut label = format!("  {guide}{}", trunc(&kid.comm, 26));
                    if !kid.mem.from_rollup {
                        label.push_str(" rss");
                    }
                    rows.push(Row::new(vec![
                        Cell::from(Line::from(label).style(Style::default().fg(p.dim))),
                        Cell::from(""),
                        cpu_cell(kid.cpu_pct, &p),
                        mem_cell(kid.mem.pss, max_pss, &p),
                        io_cell(kid.io.read_bytes + kid.io.write_bytes, &p),
                        power_cell(kid.power_w, &p),
                        Cell::from(
                            Line::from(Span::styled(
                                kid.pid.to_string(),
                                Style::default().fg(p.dim),
                            ))
                            .alignment(Alignment::Right),
                        ),
                    ]));
                }
            }
        }
    }

    let sorted = |key: crate::state::SortKey| -> Style {
        if state.sort == key {
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.dim)
        }
    };
    let header = Row::new(vec![
        Cell::from(Span::styled("Name", sorted(crate::state::SortKey::Name))),
        Cell::from(
            Line::from(Span::styled("N", Style::default().fg(p.dim))).alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("CPU", sorted(crate::state::SortKey::Cpu)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("Memory", sorted(crate::state::SortKey::Pss)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("Disk", sorted(crate::state::SortKey::Io)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("Power", sorted(crate::state::SortKey::Power)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("PID", sorted(crate::state::SortKey::Pid)))
                .alignment(Alignment::Right),
        ),
    ])
    .style(Style::default().bg(p.base));

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Length(3),
            Constraint::Length(11),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Min(0),
        ],
    )
    .header(header)
    .block(panel(state, "Processes"))
    .row_highlight_style(Style::default().bg(p.surface));

    let mut ts = TableState::default();
    ts.select(selected_row);
    f.render_stateful_widget(table, area, &mut ts);
    selected_row
}

fn draw_action_bar(f: &mut Frame, state: &AppState, area: Rect, _sel: Option<usize>) {
    let p = state.palette;
    let compact = area.width < 100;
    let line = if let Some(g) = state.selected_group() {
        let exe = g
            .pids
            .first()
            .and_then(|pid| state.snapshot.process_by_pid(*pid))
            .and_then(|proc| proc.exe.clone())
            .unwrap_or_default();
        let mut spans = vec![
            Span::styled(
                format!(" {} ", g.label()),
                Style::default()
                    .fg(p.base)
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} pids  ", g.count()),
                Style::default().fg(p.dim),
            ),
            Span::styled("k", Style::default().fg(p.text)),
            Span::styled(" kill  ", Style::default().fg(p.dim)),
            Span::styled("t", Style::default().fg(p.text)),
            Span::styled(" term  ", Style::default().fg(p.dim)),
            Span::styled("z", Style::default().fg(p.text)),
            Span::styled(" stop  ", Style::default().fg(p.dim)),
            Span::styled("u", Style::default().fg(p.text)),
            Span::styled(" cont  ", Style::default().fg(p.dim)),
            Span::styled("+/-", Style::default().fg(p.text)),
            Span::styled(" nice  ", Style::default().fg(p.dim)),
            Span::styled("o", Style::default().fg(p.text)),
            Span::styled(" open  ", Style::default().fg(p.dim)),
        ];
        if !compact {
            spans.push(Span::styled("p", Style::default().fg(p.text)));
            spans.push(Span::styled(" pin  ", Style::default().fg(p.dim)));
            spans.push(Span::styled(trunc(&exe, 40), Style::default().fg(p.dim)));
        }
        Line::from(spans)
    } else {
        Line::from(Span::styled(
            " select a group (j/k) for actions",
            Style::default().fg(p.dim),
        ))
    };
    f.render_widget(Paragraph::new(line), area);
}

// ── memory ───────────────────────────────────────────────────────────────────

fn draw_memory(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let mem = &s.mem;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // stacked bar + summary
            Constraint::Length(5), // three panels
            Constraint::Min(6),    // treemap
            Constraint::Length(2), // reclaim bar
        ])
        .split(area);

    // Stacked bar.
    let block = panel(state, "Memory");
    let inner = block.inner(chunks[0]);
    f.render_widget(block, chunks[0]);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(inner);
    draw_stackbar(f, state, rows[0]);
    let un = mem.unattributed(s.sum_pss);
    let summary = format!(
        " {} really used  ·  {} cache you can reclaim  ·  {} available ",
        human_kb(mem.used_real),
        human_kb(mem.cache_reclaimable),
        human_kb(mem.available),
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            summary,
            Style::default().fg(p.text),
        ))),
        rows[1],
    );

    // Three panels.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(chunks[1]);
    let narrow = cols[0].width < 34;
    draw_stat_panel(
        f,
        state,
        cols[0],
        "Really used",
        human_kb(mem.used_real),
        &if narrow {
            format!("{} of total", pct(mem.used_fraction() * 100.0))
        } else {
            format!(
                "{} of total · PSI {:.1}",
                pct(mem.used_fraction() * 100.0),
                s.pressure.gauge()
            )
        },
        p.accent,
    );
    draw_stat_panel(
        f,
        state,
        cols[1],
        "Reclaimable",
        human_kb(mem.cache_reclaimable),
        &if narrow {
            "page cache + slab".to_string()
        } else {
            format!(
                "page cache + buffers + slab · shared {}",
                human_kb(mem.shared)
            )
        },
        p.ok,
    );
    draw_stat_panel(
        f,
        state,
        cols[2],
        "Kernel + unattributed",
        human_kb(mem.kernel + un),
        &if narrow {
            format!("kernel {} · unattr. {}", human_kb(mem.kernel), human_kb(un))
        } else {
            format!(
                "kernel {} · unattributed {} · Σ PSS {}",
                human_kb(mem.kernel),
                human_kb(un),
                human_kb(s.sum_pss)
            )
        },
        p.warn,
    );

    // Treemap.
    draw_treemap(f, state, chunks[2]);

    // Reclaim bar: keys on one line, context on the next.
    let swap_blocker = pulsar_core::actions::swap_flush_blocker(mem);
    let mut keys = vec![
        Span::styled("reclaim: ", Style::default().fg(p.dim)),
        Span::styled("c", Style::default().fg(p.text)),
        Span::styled(" drop caches  ", Style::default().fg(p.dim)),
        Span::styled("C", Style::default().fg(p.text)),
        Span::styled(" drop all  ", Style::default().fg(p.dim)),
        Span::styled("m", Style::default().fg(p.text)),
        Span::styled(" compact  ", Style::default().fg(p.dim)),
    ];
    if swap_blocker.is_none() {
        keys.push(Span::styled("w", Style::default().fg(p.text)));
        keys.push(Span::styled(" flush swap  ", Style::default().fg(p.dim)));
    }
    if !s.rapl_readable {
        keys.push(Span::styled("p", Style::default().fg(p.text)));
        keys.push(Span::styled(
            " enable power metrics",
            Style::default().fg(p.dim),
        ));
    }
    let mut notes = Vec::new();
    if let Some(reason) = &swap_blocker {
        notes.push(format!("flush swap disabled: {reason}"));
    }
    if !s.rapl_readable {
        notes.push("power metrics need the udev rule (pkexec)".to_string());
    }
    if notes.is_empty() {
        notes.push("actions run via pkexec and are logged in the activity drawer".to_string());
    }
    f.render_widget(
        Paragraph::new(vec![
            Line::from(keys),
            Line::from(Span::styled(
                notes.join("  ·  "),
                Style::default().fg(p.dim),
            )),
        ])
        .wrap(Wrap { trim: true }),
        chunks[3],
    );
}

fn draw_stackbar(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let mem = &state.snapshot.mem;
    let total = mem.total.max(1) as f64;
    // Segments: apps(used_real-kernel), shared, kernel, cache, free.
    let apps = mem.used_by_apps() as f64;
    let segs: Vec<(f64, Color, &str)> = vec![
        (apps, p.accent, "used"),
        (mem.shared as f64, blend(p.accent, p.text, 0.35), "shared"),
        (mem.kernel as f64, p.warn, "kernel"),
        (mem.cache_reclaimable as f64, p.ok, "cache"),
        (mem.free as f64, p.border, "free"),
    ];
    let sum: f64 = segs.iter().map(|s| s.0).sum();
    let scale = if sum > 0.0 {
        area.width as f64 / sum
    } else {
        0.0
    };
    let mut x = area.x;
    let buf = f.buffer_mut();
    for (val, color, label) in &segs {
        let w = (val * scale).round() as u16;
        for dx in 0..w {
            if let Some(cell) = buf.cell_mut((x + dx, area.y)) {
                cell.set_bg(*color);
                cell.set_fg(p.base);
                cell.set_symbol(" ");
            }
        }
        // Label at the segment start if there is room.
        let pctv = if total > 0.0 {
            val / total * 100.0
        } else {
            0.0
        };
        if w >= 9 {
            let l = if *label == "kernel" {
                "kernel".to_string()
            } else {
                format!("{} {:.0}%", label, pctv)
            };
            buf.set_stringn(
                x + 1,
                area.y,
                &l,
                w.saturating_sub(2) as usize,
                Style::default()
                    .fg(p.base)
                    .bg(*color)
                    .add_modifier(Modifier::BOLD),
            );
        }
        x += w;
    }
}

fn draw_stat_panel(
    f: &mut Frame,
    state: &AppState,
    area: Rect,
    title: &str,
    big: String,
    sub: &str,
    color: Color,
) {
    let p = state.palette;
    let block = panel(state, title);
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                big,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(sub.to_string(), Style::default().fg(p.dim))),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}

fn draw_treemap(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let block = panel(state, "Memory treemap by app (PSS)");
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < 4 || inner.height < 3 {
        return;
    }
    let mut groups: Vec<&Group> = state.snapshot.groups.iter().filter(|g| g.pss > 0).collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.pss));
    let total = groups.len();
    let top_n = 20.min(total);
    let mut entries: Vec<(String, u64, Color)> = groups
        .iter()
        .take(top_n)
        .enumerate()
        .map(|(i, g)| (g.name.clone(), g.pss, p.category(i)))
        .collect();
    if total > top_n {
        let rest: u64 = groups[top_n..].iter().map(|g| g.pss).sum();
        entries.push((format!("other ({})", total - top_n), rest, p.category(7)));
    }
    let values: Vec<f64> = entries.iter().map(|e| e.1 as f64).collect();
    let rects = squarify(&values, inner.width as f64, inner.height as f64);
    // Squarify can leave a rounding remainder; stretch to fill the panel.
    let used_h = rects.iter().map(|r| r.y + r.h).fold(0.0_f64, f64::max);
    let used_w = rects.iter().map(|r| r.x + r.w).fold(0.0_f64, f64::max);
    let sy = if used_h > 0.0 {
        inner.height as f64 / used_h
    } else {
        1.0
    };
    let sx = if used_w > 0.0 {
        inner.width as f64 / used_w
    } else {
        1.0
    };
    let buf = f.buffer_mut();
    for (i, (name, pss, color)) in entries.iter().enumerate() {
        let Some(r) = rects.get(i) else { continue };
        if r.w < 1.0 || r.h < 1.0 {
            continue;
        }
        let x0 = inner.x + (r.x * sx).round() as u16;
        let y0 = inner.y + (r.y * sy).round() as u16;
        // 1-cell gap instead of borders.
        let w = ((r.w * sx).round() as u16)
            .saturating_sub(1)
            .max(1)
            .min(inner.right().saturating_sub(x0));
        let h = ((r.h * sy).round() as u16)
            .saturating_sub(1)
            .max(1)
            .min(inner.bottom().saturating_sub(y0));
        for yy in 0..h {
            for xx in 0..w {
                if let Some(cell) = buf.cell_mut((x0 + xx, y0 + yy)) {
                    cell.set_bg(*color);
                    cell.set_fg(p.base);
                    cell.set_symbol(" ");
                }
            }
        }
        // Labels only where they actually fit.
        if w >= 8 && h >= 2 {
            let label = format!(" {}", trunc(name, w as usize - 2));
            buf.set_string(
                x0,
                y0,
                &label,
                Style::default()
                    .fg(p.base)
                    .bg(*color)
                    .add_modifier(Modifier::BOLD),
            );
            if h >= 3 {
                let val = format!(" {}", human_bytes(*pss as f64 * 1024.0));
                buf.set_string(x0, y0 + 1, &val, Style::default().fg(p.base).bg(*color));
            }
        }
    }
}

// ── ports ────────────────────────────────────────────────────────────────────

fn draw_ports(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(area);

    let listening = state.port_count();
    let exposed = state
        .snapshot
        .sockets
        .iter()
        .filter(|s| state.port_matches(s) && s.is_wildcard())
        .count();
    let unowned = state
        .snapshot
        .sockets
        .iter()
        .filter(|s| state.port_matches(s) && s.pid.is_none())
        .count();
    let compact = chunks[0].width < 100;
    let mut tool = if compact {
        vec![
            Span::styled(format!("{listening}"), Style::default().fg(p.text)),
            Span::styled(" listen  ", Style::default().fg(p.dim)),
            Span::styled(format!("{exposed}"), Style::default().fg(p.crit)),
            Span::styled(" exposed  ", Style::default().fg(p.dim)),
            Span::styled(format!("{unowned}"), Style::default().fg(p.dim)),
            Span::styled(" unowned  ", Style::default().fg(p.dim)),
            Span::styled(
                if state.ports_grouped {
                    "grouped"
                } else {
                    "flat"
                },
                Style::default().fg(p.accent),
            ),
            Span::styled("  j/k · enter · u · x", Style::default().fg(p.dim)),
        ]
    } else {
        vec![
            Span::styled("listening ", Style::default().fg(p.dim)),
            Span::styled(format!("{listening}"), Style::default().fg(p.text)),
            Span::styled("  exposed ", Style::default().fg(p.dim)),
            Span::styled(format!("{exposed}"), Style::default().fg(p.crit)),
            Span::styled("  unowned ", Style::default().fg(p.dim)),
            Span::styled(format!("{unowned}"), Style::default().fg(p.dim)),
            Span::styled(
                if state.ports_grouped {
                    "   grouped"
                } else {
                    "   ungrouped"
                },
                Style::default().fg(p.accent),
            ),
            Span::styled("  j/k", Style::default().fg(p.text)),
            Span::styled(" move  ", Style::default().fg(p.dim)),
            Span::styled("enter", Style::default().fg(p.text)),
            Span::styled(" expand  ", Style::default().fg(p.dim)),
            Span::styled("u", Style::default().fg(p.text)),
            Span::styled(" group  ", Style::default().fg(p.dim)),
            Span::styled("x", Style::default().fg(p.text)),
            Span::styled(" kill", Style::default().fg(p.dim)),
        ]
    };
    if !state.search.is_empty() {
        tool.push(Span::styled("  /", Style::default().fg(p.accent)));
        tool.push(Span::styled(
            state.search.clone(),
            Style::default().fg(p.text),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(tool)), chunks[0]);

    if state.ports_grouped {
        draw_ports_grouped(f, state, chunks[1]);
    } else {
        draw_ports_flat(f, state, chunks[1]);
    }
}

fn scope_style(addr: &str, p: &crate::theme_map::Palette) -> (&'static str, Color) {
    if addr == "0.0.0.0" || addr == "::" {
        ("all ifaces", p.crit)
    } else if addr == "127.0.0.1" || addr == "::1" {
        ("localhost", p.dim)
    } else {
        ("interface", p.warn)
    }
}

fn draw_ports_grouped(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let wide = area.width >= 100;
    let groups = state.port_groups();
    if groups.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "no matching open ports",
                Style::default().fg(p.dim),
            )))
            .block(panel(state, "Open ports (grouped by process)")),
            area,
        );
        return;
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut selected_row: Option<usize> = None;
    for (gi, g) in groups.iter().enumerate() {
        if gi == state.ports_selected {
            selected_row = Some(rows.len());
        }
        let selected = gi == state.ports_selected;
        let scope_color = if g.all_ifaces > 0 {
            p.crit
        } else if g.iface > 0 {
            p.warn
        } else {
            p.dim
        };
        let mut cells = vec![
            Cell::from(Line::from(g.label()).style(Style::default().fg(p.text))),
            Cell::from(Line::from(g.ports(28)).style(Style::default().fg(p.dim))),
            Cell::from(Line::from(g.scopes()).style(Style::default().fg(scope_color))),
            Cell::from(
                Line::from(
                    g.pid
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                )
                .alignment(Alignment::Right)
                .style(Style::default().fg(p.dim)),
            ),
        ];
        if wide {
            cells.push(Cell::from(Line::from(
                g.cwd
                    .as_deref()
                    .map(|d| trunc(d, 40))
                    .unwrap_or_else(|| "—".to_string()),
            )));
        }
        let mut row = Row::new(cells);
        if selected {
            row = row.style(Style::default().bg(p.surface));
        }
        rows.push(row);

        if state.ports_expanded.contains(&g.key) {
            let n = g.sockets.len();
            for (j, s) in g.sockets.iter().enumerate() {
                let guide = if j + 1 == n { "└─ " } else { "├─ " };
                let (scope, color) = scope_style(&s.addr, &p);
                let mut cells = vec![
                    Cell::from(Line::from(format!("  {guide}{} {}", s.proto, s.endpoint()))),
                    Cell::from(Line::from(s.state.clone()).style(Style::default().fg(p.dim))),
                    Cell::from(Line::from(scope).style(Style::default().fg(color))),
                    Cell::from(""),
                ];
                if wide {
                    cells.push(Cell::from(""));
                }
                rows.push(Row::new(cells));
            }
        }
    }

    let mut head = vec![
        Cell::from(Span::styled("Process", Style::default().fg(p.dim))),
        Cell::from(Span::styled("Ports / state", Style::default().fg(p.dim))),
        Cell::from(Span::styled("Scope", Style::default().fg(p.dim))),
        Cell::from(
            Line::from(Span::styled("PID", Style::default().fg(p.dim))).alignment(Alignment::Right),
        ),
    ];
    if wide {
        head.push(Cell::from(Span::styled(
            "Working dir",
            Style::default().fg(p.dim),
        )));
    }

    let widths: Vec<Constraint> = if wide {
        vec![
            Constraint::Min(20),
            Constraint::Min(16),
            Constraint::Length(16),
            Constraint::Length(7),
            Constraint::Min(16),
            Constraint::Min(0),
        ]
    } else {
        vec![
            Constraint::Min(18),
            Constraint::Min(14),
            Constraint::Length(14),
            Constraint::Length(7),
            Constraint::Min(0),
        ]
    };

    let table = Table::new(rows, widths)
        .header(Row::new(head).style(Style::default().bg(p.base)))
        .block(panel(state, "Open ports (grouped by process)"))
        .row_highlight_style(Style::default().bg(p.surface));
    let mut ts = TableState::default();
    ts.select(selected_row);
    f.render_stateful_widget(table, area, &mut ts);
}

fn draw_ports_flat(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let wide = area.width >= 120;
    let mut rows_data: Vec<&pulsar_core::sockets::ListeningSocket> = state
        .snapshot
        .sockets
        .iter()
        .filter(|s| state.port_matches(s))
        .collect();
    rows_data.sort_by_key(|s| s.port);

    if rows_data.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "no matching open ports",
                Style::default().fg(p.dim),
            )))
            .block(panel(state, "Open ports")),
            area,
        );
        return;
    }

    let mut head = vec![
        Cell::from(Span::styled("Proto", Style::default().fg(p.dim))),
        Cell::from(Span::styled("Local address", Style::default().fg(p.dim))),
        Cell::from(Span::styled("State", Style::default().fg(p.dim))),
        Cell::from(Span::styled("Process", Style::default().fg(p.dim))),
        Cell::from(
            Line::from(Span::styled("PID", Style::default().fg(p.dim))).alignment(Alignment::Right),
        ),
        Cell::from(Span::styled("Scope", Style::default().fg(p.dim))),
    ];
    if wide {
        head.push(Cell::from(Span::styled(
            "Working dir",
            Style::default().fg(p.dim),
        )));
    }

    let rows: Vec<Row> = rows_data
        .iter()
        .map(|s| {
            let (scope, color) = scope_style(&s.addr, &p);
            let mut cells = vec![
                Cell::from(Line::from(s.proto.clone()).style(Style::default().fg(p.accent))),
                Cell::from(Line::from(s.endpoint()).style(Style::default().fg(p.text))),
                Cell::from(Line::from(s.state.clone()).style(Style::default().fg(p.dim))),
                Cell::from(
                    Line::from(s.comm.clone().unwrap_or_else(|| "—".to_string()))
                        .style(Style::default().fg(p.text)),
                ),
                Cell::from(
                    Line::from(
                        s.pid
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    )
                    .alignment(Alignment::Right)
                    .style(Style::default().fg(p.dim)),
                ),
                Cell::from(Line::from(scope).style(Style::default().fg(color))),
            ];
            if wide {
                cells.push(Cell::from(Line::from(
                    s.cwd
                        .as_deref()
                        .map(|d| trunc(d, 44))
                        .unwrap_or_else(|| "—".to_string()),
                )));
            }
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = if wide {
        vec![
            Constraint::Length(6),
            Constraint::Length(26),
            Constraint::Length(8),
            Constraint::Length(22),
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Min(16),
            Constraint::Min(0),
        ]
    } else {
        vec![
            Constraint::Length(5),
            Constraint::Length(20),
            Constraint::Length(7),
            Constraint::Length(16),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Min(0),
        ]
    };

    let table = Table::new(rows, widths)
        .header(Row::new(head).style(Style::default().bg(p.base)))
        .block(panel(state, "Open ports (TCP listen / UDP bound)"))
        .row_highlight_style(Style::default().bg(p.surface));
    let mut ts = TableState::default();
    ts.select(Some(
        state.ports_selected.min(rows_data.len().saturating_sub(1)),
    ));
    f.render_stateful_widget(table, area, &mut ts);
}

// ── power ────────────────────────────────────────────────────────────────────

fn draw_power(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let power = s.power;
    // The Power view reports a 30-second rolling average, not the instantaneous tick.
    let n = avg_window_samples(state);
    let avg_total = avg_last(&s.series.power, n);
    let avg_pkg = avg_last(&s.series.pkg, n);
    let avg_dynamic = avg_last(&s.series.cpu_dynamic, n);
    let avg_gpu = avg_last(&s.series.gpu, n);
    let avg_battery = avg_last(&s.series.battery, n);
    let gpu_available = power.gpu_w.is_some() || avg_gpu > 0.0;
    let battery_available = power.battery_w.is_some() || avg_battery > 0.0;

    let compact = area.height < 20;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if compact { 4 } else { 5 }),
            Constraint::Length(if compact { 5 } else { 9 }),
            Constraint::Min(5),
        ])
        .split(area);

    // Summary.
    let block = panel(state, "Power · 30s average");
    let inner = block.inner(chunks[0]);
    f.render_widget(block, chunks[0]);
    let mut lines = vec![Line::from(vec![
        Span::styled("total ", Style::default().fg(p.dim)),
        Span::styled(
            human_watts(avg_total),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  (30s avg)  ", Style::default().fg(p.dim)),
        Span::styled("package ", Style::default().fg(p.dim)),
        Span::styled(human_watts(avg_pkg), Style::default().fg(p.text)),
        Span::styled("   dynamic ", Style::default().fg(p.dim)),
        Span::styled(human_watts(avg_dynamic), Style::default().fg(p.text)),
    ])];
    let mut extra = vec![
        Span::styled("baseline ", Style::default().fg(p.dim)),
        Span::styled(human_watts(power.baseline_w), Style::default().fg(p.text)),
    ];
    if gpu_available {
        extra.push(Span::styled("   gpu ", Style::default().fg(p.dim)));
        extra.push(Span::styled(
            human_watts(avg_gpu),
            Style::default().fg(p.text),
        ));
    }
    if battery_available {
        extra.push(Span::styled("   battery ", Style::default().fg(p.dim)));
        extra.push(Span::styled(
            human_watts(avg_battery),
            Style::default().fg(p.text),
        ));
    }
    lines.push(Line::from(extra));
    f.render_widget(Paragraph::new(lines), inner);

    // Braille area graph with a vertical gradient.
    let ymax = s
        .series
        .power
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(5.0)
        * 1.25;
    let title = format!("System watts over time · {ymax:.0} W scale");
    let block = panel(state, &title);
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);
    if inner.width > 2 && inner.height > 0 {
        let graph = crate::graph::braille_area_max(
            &s.series.power,
            inner.width as usize,
            inner.height as usize,
            p.accent,
            blend(p.accent, p.base, 0.90),
            ymax,
        );
        f.render_widget(Paragraph::new(graph), inner);
    }

    // Top consumers: one bar colour, values aligned in a column, one footnote.
    let block = panel(state, "Top power consumers (30s avg)");
    let inner = block.inner(chunks[2]);
    f.render_widget(block, chunks[2]);
    let mut groups: Vec<(&Group, f64)> = state
        .snapshot
        .groups
        .iter()
        .map(|g| (g, avg_last(&g.spark, n)))
        .filter(|(_, a)| *a > 0.0005)
        .collect();
    groups.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let maxp = groups.first().map(|(_, a)| *a).unwrap_or(1.0).max(0.001);

    let foot_h = if inner.height >= 8 { 2 } else { 0 };
    let list_h = inner.height.saturating_sub(foot_h) as usize;
    let name_w = 26u16.min(inner.width.saturating_sub(14));
    let value_w = 8u16;
    let bar_w = inner.width.saturating_sub(name_w + value_w + 2) as usize;
    let mut lines = Vec::new();
    for (g, ap) in groups.iter().take(list_h) {
        let mut spans = vec![
            Span::styled(
                format!(
                    "{:<width$}",
                    trunc(&g.name, name_w as usize),
                    width = name_w as usize
                ),
                Style::default().fg(p.text),
            ),
            Span::styled(" ", Style::default()),
        ];
        spans.extend(bar_spans(ap / maxp, bar_w, p.warn, p.surface));
        spans.push(Span::styled(
            format!(" {:>width$}", human_watts(*ap), width = value_w as usize),
            Style::default().fg(p.text),
        ));
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "no attributed power yet",
            Style::default().fg(p.dim),
        )));
    }
    let sum_attr: f64 = state.snapshot.processes.iter().map(|pr| pr.power_w).sum();
    let mut foot = vec![Line::from(Span::styled(
        format!(
            "Σ apps {}{}",
            human_watts(sum_attr),
            if gpu_available {
                format!("   ·   gpu (system-only) {}", human_watts(avg_gpu))
            } else {
                String::new()
            }
        ),
        Style::default().fg(p.dim),
    ))];
    foot.push(Line::from(Span::styled(
        "estimated: est. = RAPL energy · model = utilisation × TDP",
        Style::default().fg(p.dim),
    )));
    if foot_h == 2 {
        f.render_widget(
            Paragraph::new(foot),
            Rect {
                x: inner.x,
                y: inner.y + inner.height - 2,
                width: inner.width,
                height: 2,
            },
        );
    }
    f.render_widget(
        Paragraph::new(lines),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: list_h as u16,
        },
    );
}

// ── small helpers ────────────────────────────────────────────────────────────

/// Number of samples in the 30-second averaging window at the current refresh rate.
fn avg_window_samples(state: &AppState) -> usize {
    let dt = (state.config.refresh_ms as f64 / 1000.0).max(0.05);
    (30.0 / dt).round().max(1.0) as usize
}

/// Mean of the last `n` values (clamped to what is available).
fn avg_last(v: &[f64], n: usize) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let n = n.clamp(1, v.len());
    v[v.len() - n..].iter().copied().sum::<f64>() / n as f64
}

fn trunc(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else if n <= 1 {
        "…".to_string()
    } else {
        let mut out: String = chars[..n - 1].iter().collect();
        out.push('…');
        out
    }
}
