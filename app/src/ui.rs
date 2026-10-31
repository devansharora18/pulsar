//! All TUI rendering: header, tabs, the four views and the shared widgets.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, BorderType, Borders, Cell, Chart, Dataset, Gauge, GraphType, Paragraph, Row,
    Sparkline, Table, TableState, Tabs, Wrap,
};
use ratatui::Frame;

use pulsar_core::group::{Group, ProcSample};
use pulsar_core::power::PowerKind;

use crate::format::{human_bytes, human_kb, human_rate, human_temp, human_watts, pct};
use crate::state::{AppState, View, ViewMode};
use crate::treemap::squarify;

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
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

    let title = Line::from(vec![
        Span::styled(
            " PULSAR ",
            Style::default()
                .fg(p.bg)
                .bg(p.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!(
                "{} used ({})",
                human_kb(mem.used_real),
                pct(mem.used_fraction() * 100.0)
            ),
            Style::default().fg(p.fg),
        ),
        Span::styled(
            format!("  ·  {} cache reclaimable", human_kb(mem.cache_reclaimable)),
            Style::default().fg(p.green),
        ),
        Span::styled(
            format!("  ·  {} available", human_kb(mem.available)),
            Style::default().fg(p.dim),
        ),
        Span::styled(
            format!("  ·  cpu {}", pct(s.cpu.overall_usage_pct)),
            Style::default().fg(p.dim),
        ),
        Span::styled(
            format!("  ·  {}", human_watts(power.total_w)),
            Style::default().fg(p.dim),
        ),
        Span::styled(
            match power.kind {
                PowerKind::Rapl => " est.",
                PowerKind::Model => " model",
                PowerKind::Unavailable => "",
            },
            Style::default().fg(p.yellow),
        ),
        Span::styled(
            format!("  ·  {} procs", s.proc_count),
            Style::default().fg(p.dim),
        ),
    ]);
    f.render_widget(
        Paragraph::new(vec![
            title,
            Line::from(Span::styled(
                format!(
                    " {} · {} cpu · {}",
                    state.snapshot.cpu.model,
                    state.snapshot.cpu.cores.len(),
                    state.theme.name,
                ),
                Style::default().fg(p.dim),
            )),
        ])
        .style(Style::default().bg(p.bg)),
        area,
    );
}

fn draw_tabs(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let titles: Vec<Line> = View::all()
        .iter()
        .enumerate()
        .map(|(i, v)| Line::from(format!(" {} {} ", i + 1, v.label())))
        .collect();
    let selected = View::all()
        .iter()
        .position(|v| *v == state.view)
        .unwrap_or(0);
    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(p.dim).bg(p.bg))
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
        Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(p.yellow),
        ))
    } else {
        Line::from(vec![
            Span::styled(" 1-5", Style::default().fg(p.accent)),
            Span::styled(" view  ", Style::default().fg(p.dim)),
            Span::styled("/", Style::default().fg(p.accent)),
            Span::styled(" search  ", Style::default().fg(p.dim)),
            Span::styled("j/k", Style::default().fg(p.accent)),
            Span::styled(" move  ", Style::default().fg(p.dim)),
            Span::styled("enter", Style::default().fg(p.accent)),
            Span::styled(" expand  ", Style::default().fg(p.dim)),
            Span::styled("g/f/t", Style::default().fg(p.accent)),
            Span::styled(" view  ", Style::default().fg(p.dim)),
            Span::styled("x", Style::default().fg(p.accent)),
            Span::styled(" kill  ", Style::default().fg(p.dim)),
            Span::styled("s", Style::default().fg(p.accent)),
            Span::styled(" settings  ", Style::default().fg(p.dim)),
            Span::styled("?", Style::default().fg(p.accent)),
            Span::styled(" help  ", Style::default().fg(p.dim)),
            Span::styled("q", Style::default().fg(p.accent)),
            Span::styled(" quit", Style::default().fg(p.dim)),
        ])
    };
    f.render_widget(Paragraph::new(line).style(Style::default().bg(p.bg)), area);
}

fn panel(state: &AppState, title: &str) -> Block<'static> {
    let p = state.palette;
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.border))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(p.bg))
}

// ── overview ─────────────────────────────────────────────────────────────────

fn draw_overview(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(cols[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(8),
            Constraint::Min(6),
        ])
        .split(cols[1]);

    // Big gauges.
    let gblock = panel(state, "Resources");
    let inner = gblock.inner(left[0]);
    f.render_widget(gblock, left[0]);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1); 4])
        .split(inner);
    let gauges = [
        (
            "CPU ",
            s.cpu.overall_usage_pct / 100.0,
            p.accent,
            pct(s.cpu.overall_usage_pct),
        ),
        (
            "MEM ",
            if s.mem.total > 0 {
                s.mem.used_real as f64 / s.mem.total as f64
            } else {
                0.0
            },
            p.magenta,
            human_kb(s.mem.used_real),
        ),
        (
            "SWAP",
            if s.mem.swap_total > 0 {
                s.mem.swap_used as f64 / s.mem.swap_total as f64
            } else {
                0.0
            },
            p.yellow,
            human_kb(s.mem.swap_used),
        ),
        (
            "PWR ",
            (s.power.total_w / state.config.tdp_w.map(|t| t * 2.0).unwrap_or(65.0)).clamp(0.0, 1.0),
            p.green,
            human_watts(s.power.total_w),
        ),
    ];
    for (i, (label, ratio, color, val)) in gauges.iter().enumerate() {
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(*color).bg(p.border))
                .ratio(ratio.clamp(0.0, 1.0))
                .label(Span::styled(
                    format!("{label} {val}"),
                    Style::default().fg(p.fg).add_modifier(Modifier::BOLD),
                )),
            rows[i],
        );
    }

    // Per-core heat grid (fixed 2 rows of 8 cells at 3 lines each).
    draw_core_panel(f, state, left[1]);

    // System load / temps.
    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" load  ", Style::default().fg(p.dim)),
            Span::styled(
                format!("{:.2} {:.2} {:.2}", s.cpu.load1, s.cpu.load5, s.cpu.load15),
                Style::default().fg(p.fg),
            ),
        ]),
        Line::from(vec![
            Span::styled(" temp  ", Style::default().fg(p.dim)),
            Span::styled(temp_line(state), Style::default().fg(p.orange)),
        ]),
    ]);
    let lb = panel(state, "Load & thermals");
    let linner = lb.inner(left[2]);
    f.render_widget(lb, left[2]);
    f.render_widget(info, linner);

    draw_top_mem(f, state, right[0]);
    draw_net(f, state, right[1]);
    draw_filesystems(f, state, right[2]);
}

fn draw_core_panel(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let block = panel(state, "CPU cores");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let cores = &state.snapshot.cpu.cores;
    if cores.is_empty() || inner.width < 8 {
        return;
    }
    let cols = 8u16;
    let cw = inner.width / cols;
    let rows = (cores.len() as u16).div_ceil(cols);
    let ch = (inner.height / rows.max(1)).clamp(1, 3);

    let buf = f.buffer_mut();
    for (i, c) in cores.iter().enumerate() {
        let cx = inner.x + (i as u16 % cols) * cw;
        let cy = inner.y + (i as u16 / cols) * ch;
        let color = p.heat(c.usage_pct / 100.0);
        for dy in 0..ch {
            for dx in 0..cw.saturating_sub(2) {
                if let Some(cell) = buf.cell_mut((cx + dx, cy + dy)) {
                    cell.set_bg(color);
                    cell.set_style(Style::default().bg(color));
                }
            }
        }
        let usage = format!("{:.0}", c.usage_pct);
        buf.set_stringn(
            cx,
            cy,
            &usage,
            (cw - 2) as usize,
            Style::default()
                .fg(p.bg)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        );
        if ch >= 2 {
            if let Some(freq) = c.freq_mhz {
                let fs = short_freq(freq);
                buf.set_stringn(
                    cx,
                    cy + 1,
                    &fs,
                    (cw - 2) as usize,
                    Style::default().fg(p.bg).bg(color),
                );
            }
        }
    }
}

fn short_freq(mhz: u64) -> String {
    if mhz >= 1000 {
        format!("{:.1}G", mhz as f64 / 1000.0)
    } else {
        format!("{mhz}M")
    }
}

fn draw_top_mem(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let block = panel(state, "Top memory (PSS)");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut groups: Vec<&Group> = state.snapshot.groups.iter().filter(|g| g.pss > 0).collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.pss));
    let max = groups.first().map(|g| g.pss).unwrap_or(1).max(1);
    let mut lines = Vec::new();
    for g in groups.iter().take(inner.height as usize) {
        let frac = g.pss as f64 / max as f64;
        let barw = 22usize;
        let filled = (frac * barw as f64).round() as usize;
        let bar = "█".repeat(filled) + &"░".repeat(barw - filled);
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<26}", trunc(&g.label(), 26)),
                Style::default().fg(p.fg),
            ),
            Span::styled(bar, Style::default().fg(p.series(0))),
            Span::styled(
                format!(" {:>9}", human_bytes(g.pss as f64 * 1024.0)),
                Style::default().fg(p.magenta),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn temp_line(state: &AppState) -> String {
    let mut parts = Vec::new();
    for t in &state.snapshot.temps {
        if (t.chip == "k10temp" || t.chip == "coretemp") && parts.is_empty() {
            parts.push(format!("cpu {}", human_temp(t.temp_c)));
        } else if t.chip == "amdgpu" {
            parts.push(format!("gpu {}", human_temp(t.temp_c)));
        }
        if parts.len() >= 2 {
            break;
        }
    }
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join("  ·  ")
    }
}

fn draw_net(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let s = &state.snapshot;
    let block = panel(state, "Network");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(2)])
        .split(inner);
    let mut lines = Vec::new();
    for iface in s.net.iter().filter(|i| i.up || i.rx_bps > 0.0).take(2) {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<10}", iface.name), Style::default().fg(p.fg)),
            Span::styled(
                format!("↓{:<11}", human_rate(iface.rx_bps)),
                Style::default().fg(p.blue),
            ),
            Span::styled(
                format!("↑{}", human_rate(iface.tx_bps)),
                Style::default().fg(p.cyan),
            ),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " no active interface",
            Style::default().fg(p.dim),
        )));
    }
    let totals: Vec<u64> = s
        .series
        .net_rx
        .iter()
        .map(|v| *v as u64)
        .chain(s.series.net_tx.iter().map(|v| *v as u64))
        .collect();
    f.render_widget(Paragraph::new(lines), rows[0]);
    f.render_widget(
        Sparkline::default()
            .data(&totals)
            .style(Style::default().fg(p.blue)),
        rows[1],
    );
}

fn draw_filesystems(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let block = panel(state, "Filesystems");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = Vec::new();
    for fs in state
        .snapshot
        .filesystems
        .iter()
        .take(inner.height as usize)
    {
        let frac = fs.used_fraction();
        let barw = 14;
        let filled = (frac * barw as f64).round() as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(barw - filled);
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<16}", trunc(&fs.mount, 16)),
                Style::default().fg(p.fg),
            ),
            Span::styled(bar, Style::default().fg(p.heat(frac))),
            Span::styled(
                format!(" {:>9}", human_bytes(fs.used_bytes as f64)),
                Style::default().fg(p.dim),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
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
            Style::default().fg(p.fg),
        ),
    ];
    if state.hide_kernel {
        tool.push(Span::styled(
            "  [kernel hidden]",
            Style::default().fg(p.yellow),
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
            Style::default().fg(p.fg).add_modifier(Modifier::BOLD),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(tool)), chunks[0]);

    let selected_idx = draw_process_table(f, state, chunks[1]);
    draw_action_bar(f, state, chunks[2], selected_idx);
}

fn draw_process_table(f: &mut Frame, state: &mut AppState, area: Rect) -> Option<usize> {
    let p = state.palette;
    let (visible, child_map) = {
        let keys = state.visible_keys();
        let snap = state.snapshot.clone();
        let mut groups: Vec<Group> = keys
            .iter()
            .filter_map(|k| snap.groups.iter().find(|g| &g.key == k).cloned())
            .collect();
        // Pinned first.
        groups.sort_by_key(|g| !state.is_pinned(&g.key));
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
        (groups, child_map)
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut selected_row: Option<usize> = None;
    for g in &visible {
        let is_sel = state.selected.as_deref() == Some(g.key.as_str());
        if is_sel {
            selected_row = Some(rows.len());
        }
        let pinned = state.is_pinned(&g.key);
        let name = format!(
            "{}{} {}",
            if pinned { "★ " } else { "" },
            if g.count() > 1 { "▍" } else { " " },
            g.name
        );
        let style = if is_sel {
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.fg)
        };
        rows.push(Row::new(vec![
            Cell::from(Line::from(name).style(style)),
            num(g.count().to_string(), p.dim),
            num(pct(g.cpu_pct), p.fg),
            num(human_bytes(g.pss as f64 * 1024.0), p.magenta),
            num(human_bytes((g.io_read + g.io_write) as f64), p.dim),
            num(human_watts(g.power_w), p.green),
            num(
                g.pids
                    .first()
                    .map(|pid| pid.to_string())
                    .unwrap_or_default(),
                p.dim,
            ),
        ]));

        let expanded = state.view_mode == ViewMode::Tree || state.expanded.contains(&g.key);
        if expanded {
            if let Some(kids) = child_map.get(&g.key) {
                for kid in kids {
                    let engine = if kid.mem.from_rollup { "PSS" } else { "RSS" };
                    rows.push(Row::new(vec![
                        Cell::from(
                            Line::from(format!("   ╰─ {}  [{}]", trunc(&kid.comm, 30), engine))
                                .style(Style::default().fg(p.dim)),
                        ),
                        Cell::from(""),
                        num(pct(kid.cpu_pct), p.dim),
                        num(human_bytes(kid.mem.pss as f64 * 1024.0), p.dim),
                        num(
                            human_bytes((kid.io.read_bytes + kid.io.write_bytes) as f64),
                            p.dim,
                        ),
                        num(human_watts(kid.power_w), p.dim),
                        Cell::from(
                            Line::from(format!("{}", kid.pid)).style(Style::default().fg(p.dim)),
                        ),
                    ]));
                }
            }
        }
    }

    let header = Row::new(vec![
        Cell::from(Span::styled(
            "Name",
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled("N", Style::default().fg(p.accent))),
        Cell::from(
            Line::from(Span::styled("CPU", Style::default().fg(p.accent)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("Memory", Style::default().fg(p.accent)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("Disk IO", Style::default().fg(p.accent)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("Power", Style::default().fg(p.accent)))
                .alignment(Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled("PID", Style::default().fg(p.accent)))
                .alignment(Alignment::Right),
        ),
    ])
    .style(Style::default().bg(p.bg));

    let table = Table::new(
        rows,
        [
            Constraint::Min(24),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(9),
            Constraint::Length(5),
        ],
    )
    .header(header)
    .block(panel(state, "Processes"))
    .row_highlight_style(
        Style::default()
            .bg(p.accent)
            .fg(p.bg)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

    let mut ts = TableState::default();
    ts.select(selected_row);
    f.render_stateful_widget(table, area, &mut ts);
    selected_row
}

fn draw_action_bar(f: &mut Frame, state: &AppState, area: Rect, _sel: Option<usize>) {
    let p = state.palette;
    let line = if let Some(g) = state.selected_group() {
        let exe = g
            .pids
            .first()
            .and_then(|pid| state.snapshot.process_by_pid(*pid))
            .and_then(|proc| proc.exe.clone())
            .unwrap_or_default();
        Line::from(vec![
            Span::styled(
                format!(" {} ", g.label()),
                Style::default()
                    .fg(p.bg)
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} pids  ", g.count()),
                Style::default().fg(p.dim),
            ),
            Span::styled("k", Style::default().fg(p.accent)),
            Span::styled(" kill  ", Style::default().fg(p.dim)),
            Span::styled("t", Style::default().fg(p.accent)),
            Span::styled(" terminate  ", Style::default().fg(p.dim)),
            Span::styled("z", Style::default().fg(p.accent)),
            Span::styled(" suspend  ", Style::default().fg(p.dim)),
            Span::styled("u", Style::default().fg(p.accent)),
            Span::styled(" resume  ", Style::default().fg(p.dim)),
            Span::styled("+/-", Style::default().fg(p.accent)),
            Span::styled(" renice  ", Style::default().fg(p.dim)),
            Span::styled("o", Style::default().fg(p.accent)),
            Span::styled(" open  ", Style::default().fg(p.dim)),
            Span::styled("p", Style::default().fg(p.accent)),
            Span::styled(" pin  ", Style::default().fg(p.dim)),
            Span::styled(trunc(&exe, 40), Style::default().fg(p.dim)),
        ])
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
        " {} really used  ·  {} cache you can reclaim  ·  {} available  ·  kernel+unattributed {} ",
        human_kb(mem.used_real),
        human_kb(mem.cache_reclaimable),
        human_kb(mem.available),
        human_kb(mem.kernel + un),
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(summary, Style::default().fg(p.fg)))),
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
    draw_stat_panel(
        f,
        state,
        cols[0],
        "Really used",
        human_kb(mem.used_real),
        &format!(
            "{} of total · PSI {:.1}",
            pct(mem.used_fraction() * 100.0),
            s.pressure.gauge()
        ),
        p.accent,
    );
    draw_stat_panel(
        f,
        state,
        cols[1],
        "Reclaimable",
        human_kb(mem.cache_reclaimable),
        &format!(
            "page cache + buffers + slab · shared {}",
            human_kb(mem.shared)
        ),
        p.green,
    );
    draw_stat_panel(
        f,
        state,
        cols[2],
        "Kernel + unattributed",
        human_kb(mem.kernel + un),
        &format!(
            "kernel {} · unattributed {} · Σ PSS {}",
            human_kb(mem.kernel),
            human_kb(un),
            human_kb(s.sum_pss)
        ),
        p.orange,
    );

    // Treemap.
    draw_treemap(f, state, chunks[2]);

    // Reclaim bar.
    let swap_blocker = pulsar_core::actions::swap_flush_blocker(mem);
    let mut spans = vec![
        Span::styled(" reclaim: ", Style::default().fg(p.dim)),
        Span::styled("c", Style::default().fg(p.accent)),
        Span::styled(" drop caches  ", Style::default().fg(p.dim)),
        Span::styled("C", Style::default().fg(p.accent)),
        Span::styled(" drop all  ", Style::default().fg(p.dim)),
        Span::styled("m", Style::default().fg(p.accent)),
        Span::styled(" compact  ", Style::default().fg(p.dim)),
    ];
    if swap_blocker.is_none() {
        spans.push(Span::styled("w", Style::default().fg(p.accent)));
        spans.push(Span::styled(" flush swap  ", Style::default().fg(p.dim)));
    } else {
        spans.push(Span::styled(
            format!("flush swap disabled: {} ", swap_blocker.unwrap_or_default()),
            Style::default().fg(p.dim),
        ));
    }
    if !s.rapl_readable {
        spans.push(Span::styled("p", Style::default().fg(p.orange)));
        spans.push(Span::styled(
            " enable power metrics (pkexec)",
            Style::default().fg(p.dim),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), chunks[3]);
}

fn draw_stackbar(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let mem = &state.snapshot.mem;
    let total = mem.total.max(1) as f64;
    // Segments: apps(used_real-kernel), shared, kernel, cache, free.
    let apps = mem.used_by_apps() as f64;
    let segs: Vec<(f64, Color, &str)> = vec![
        (apps, p.accent, "used"),
        (mem.shared as f64, p.magenta, "shared"),
        (mem.kernel as f64, p.orange, "kernel"),
        (mem.cache_reclaimable as f64, p.green, "cache"),
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
                cell.set_fg(p.bg);
                cell.set_symbol(" ");
            }
        }
        // Label at the segment start if there is room.
        let pctv = if total > 0.0 {
            val / total * 100.0
        } else {
            0.0
        };
        if w >= 6 {
            let l = format!("{} {:.0}%", label, pctv);
            buf.set_string(
                x + 1,
                area.y,
                trunc(&l, w.saturating_sub(2) as usize),
                Style::default()
                    .fg(p.bg)
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
    groups.truncate(48);
    let values: Vec<f64> = groups.iter().map(|g| g.pss as f64).collect();
    let rects = squarify(&values, inner.width as f64, inner.height as f64);
    let buf = f.buffer_mut();
    for (i, g) in groups.iter().enumerate() {
        let Some(r) = rects.get(i) else { continue };
        if r.w < 1.0 || r.h < 1.0 {
            continue;
        }
        let color = p.series(i);
        let x0 = inner.x + r.x.round() as u16;
        let y0 = inner.y + r.y.round() as u16;
        let w = (r.w.round() as u16)
            .max(1)
            .min(inner.right().saturating_sub(x0));
        let h = (r.h.round() as u16)
            .max(1)
            .min(inner.bottom().saturating_sub(y0));
        for yy in 0..h {
            for xx in 0..w {
                if let Some(cell) = buf.cell_mut((x0 + xx, y0 + yy)) {
                    cell.set_bg(color);
                    cell.set_fg(p.bg);
                    cell.set_symbol(" ");
                }
            }
        }
        if w >= 7 {
            let label = format!(" {}", trunc(&g.name, w as usize - 2));
            buf.set_string(
                x0,
                y0,
                &label,
                Style::default()
                    .fg(p.bg)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            );
            if h >= 2 {
                let val = format!(" {}", human_bytes(g.pss as f64 * 1024.0));
                buf.set_string(x0, y0 + 1, &val, Style::default().fg(p.bg).bg(color));
            }
        }
    }
}

// ── ports ────────────────────────────────────────────────────────────────────

fn draw_ports(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let q = state.search.trim().to_lowercase();

    let mut rows_data: Vec<&pulsar_core::sockets::ListeningSocket> = state
        .snapshot
        .sockets
        .iter()
        .filter(|s| {
            if q.is_empty() {
                return true;
            }
            s.proto.to_lowercase().contains(&q)
                || s.addr.to_lowercase().contains(&q)
                || s.port.to_string().contains(&q)
                || s.comm
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&q))
                    .unwrap_or(false)
        })
        .collect();
    rows_data.sort_by_key(|s| s.port);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(area);

    let wildcard = rows_data.iter().filter(|s| s.is_wildcard()).count();
    let unowned = rows_data.iter().filter(|s| s.pid.is_none()).count();
    let mut tool = vec![
        Span::styled(" listening:", Style::default().fg(p.dim)),
        Span::styled(format!("{}", rows_data.len()), Style::default().fg(p.fg)),
        Span::styled("  exposed:", Style::default().fg(p.dim)),
        Span::styled(
            format!("{wildcard}"),
            Style::default().fg(if wildcard > 0 { p.orange } else { p.fg }),
        ),
        Span::styled("  unowned:", Style::default().fg(p.dim)),
        Span::styled(format!("{unowned}"), Style::default().fg(p.dim)),
    ];
    if !state.search.is_empty() {
        tool.push(Span::styled("  /", Style::default().fg(p.accent)));
        tool.push(Span::styled(
            state.search.clone(),
            Style::default().fg(p.fg).add_modifier(Modifier::BOLD),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(tool)), chunks[0]);

    let header = Row::new(vec![
        Cell::from(Span::styled("Proto", Style::default().fg(p.accent))),
        Cell::from(Span::styled("Local address", Style::default().fg(p.accent))),
        Cell::from(Span::styled("State", Style::default().fg(p.accent))),
        Cell::from(Span::styled("Process", Style::default().fg(p.accent))),
        Cell::from(
            Line::from(Span::styled("PID", Style::default().fg(p.accent)))
                .alignment(Alignment::Right),
        ),
        Cell::from(Span::styled("Scope", Style::default().fg(p.accent))),
        Cell::from(""),
    ])
    .style(Style::default().bg(p.bg));

    let rows: Vec<Row> = rows_data
        .iter()
        .map(|s| {
            let scope = if s.is_wildcard() {
                ("all ifaces", p.orange)
            } else if s.addr == "127.0.0.1" || s.addr == "::1" {
                ("localhost", p.green)
            } else {
                ("interface", p.dim)
            };
            Row::new(vec![
                Cell::from(Line::from(s.proto.clone()).style(Style::default().fg(p.cyan))),
                Cell::from(Line::from(s.endpoint()).style(Style::default().fg(p.fg))),
                Cell::from(Line::from(s.state.clone()).style(Style::default().fg(p.dim))),
                Cell::from(
                    Line::from(s.comm.clone().unwrap_or_else(|| "—".to_string()))
                        .style(Style::default().fg(if s.comm.is_some() { p.fg } else { p.dim })),
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
                Cell::from(Line::from(scope.0).style(Style::default().fg(scope.1))),
                Cell::from(""),
            ])
        })
        .collect();

    if rows.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " no matching open ports",
                Style::default().fg(p.dim),
            )))
            .block(panel(state, "Open ports")),
            chunks[1],
        );
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(28),
            Constraint::Length(8),
            Constraint::Length(32),
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Min(0),
        ],
    )
    .header(header)
    .block(panel(state, "Open ports (TCP listen / UDP bound)"));
    f.render_widget(table, chunks[1]);
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

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // sources + breakdown
            Constraint::Length(8), // system watts graph
            Constraint::Min(5),    // top consumers
        ])
        .split(area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);
    let kind = match power.kind {
        PowerKind::Rapl => "est. (RAPL)",
        PowerKind::Model => "model (util × TDP)",
        PowerKind::Unavailable => "unavailable",
    };
    let src = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" total   ", Style::default().fg(p.dim)),
            Span::styled(
                human_watts(avg_total),
                Style::default().fg(p.green).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  (30s avg)", Style::default().fg(p.dim)),
            Span::styled(format!("   [{kind}]"), Style::default().fg(p.yellow)),
        ]),
        Line::from(vec![
            Span::styled(" package ", Style::default().fg(p.dim)),
            Span::styled(human_watts(avg_pkg), Style::default().fg(p.fg)),
        ]),
    ]);
    let pblock = panel(state, "Power");
    let pinner = pblock.inner(chunks[0]);
    f.render_widget(pblock, chunks[0]);
    f.render_widget(src, pinner);

    let mut detail_lines = vec![
        Line::from(vec![
            Span::styled(" dynamic ", Style::default().fg(p.dim)),
            Span::styled(human_watts(avg_dynamic), Style::default().fg(p.orange)),
        ]),
        Line::from(vec![
            Span::styled(" baseline ", Style::default().fg(p.dim)),
            Span::styled(human_watts(power.baseline_w), Style::default().fg(p.fg)),
        ]),
    ];
    if gpu_available {
        detail_lines.push(Line::from(vec![
            Span::styled(" gpu     ", Style::default().fg(p.dim)),
            Span::styled(human_watts(avg_gpu), Style::default().fg(p.magenta)),
        ]));
    }
    if battery_available {
        detail_lines.push(Line::from(vec![
            Span::styled(" battery ", Style::default().fg(p.dim)),
            Span::styled(human_watts(avg_battery), Style::default().fg(p.yellow)),
        ]));
    }
    let detail = Paragraph::new(detail_lines);
    let bblock = panel(state, "Breakdown");
    let binner = bblock.inner(cols[1]);
    f.render_widget(bblock, cols[1]);
    f.render_widget(detail, binner);

    // System watts graph (line chart).
    let block = panel(state, "System watts over time");
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);
    let points: Vec<(f64, f64)> = s
        .series
        .power
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64, *v))
        .collect();
    let xmax = (points.len().saturating_sub(1)).max(1) as f64;
    let ymax = points
        .iter()
        .map(|(_, y)| *y)
        .fold(0.0_f64, f64::max)
        .max(10.0)
        * 1.15;
    let dataset = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(p.green))
        .data(&points);
    let chart = Chart::new(vec![dataset])
        .x_axis(
            Axis::default()
                .style(Style::default().fg(p.dim))
                .bounds([0.0, xmax]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(p.dim))
                .bounds([0.0, ymax])
                .labels(vec![
                    Span::raw("0"),
                    Span::styled(format!("{:.0}W", ymax / 2.0), Style::default().fg(p.dim)),
                    Span::styled(format!("{ymax:.0}W"), Style::default().fg(p.dim)),
                ]),
        );
    f.render_widget(chart, inner);

    // Top consumers.
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
    let mut lines = Vec::new();
    for (g, ap) in groups.iter().take(inner.height.saturating_sub(1) as usize) {
        let frac = ap / maxp;
        let barw = 20usize;
        let filled = (frac * barw as f64).round() as usize;
        let bar = "█".repeat(filled) + &"░".repeat(barw - filled);
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<28}", trunc(&g.name, 28)),
                Style::default().fg(p.fg),
            ),
            Span::styled(bar, Style::default().fg(p.orange)),
            Span::styled(
                format!(" {:>8}", human_watts(*ap)),
                Style::default().fg(p.green),
            ),
            Span::styled("  est.", Style::default().fg(p.yellow)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " no attributed power yet",
            Style::default().fg(p.dim),
        )));
    }
    // Reconcile: apps sum to the CPU dynamic draw; the GPU is measured at system level and
    // is not split across processes, so it is shown separately.
    let sum_attr: f64 = state.snapshot.processes.iter().map(|pr| pr.power_w).sum();
    lines.push(Line::from(Span::styled(
        format!(
            " Σ apps {}{}",
            human_watts(sum_attr),
            if gpu_available {
                format!("   ·   gpu (system-only) {}", human_watts(avg_gpu))
            } else {
                String::new()
            }
        ),
        Style::default().fg(p.dim),
    )));
    let wh: f64 = s
        .series
        .power
        .iter()
        .map(|v| v * state.config.refresh_ms as f64 / 1000.0 / 3600.0)
        .sum();
    lines.push(Line::from(Span::styled(
        format!(" cumulative since launch: {wh:.3} Wh"),
        Style::default().fg(p.dim),
    )));
    f.render_widget(Paragraph::new(lines), inner);
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

fn num(s: String, color: Color) -> Cell<'static> {
    Cell::from(
        Line::from(s)
            .alignment(Alignment::Right)
            .style(Style::default().fg(color)),
    )
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
