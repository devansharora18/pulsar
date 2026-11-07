//! Popup overlays: confirmation modal, activity log, help and settings.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::state::{AppState, Overlay, PendingKind, Setting, SETTINGS};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
    if state.confirm.is_none() && state.overlay.is_none() {
        return;
    }
    // Dim whatever is on screen; popups clear their own rect so nothing bleeds through.
    dim_backdrop(f, area, state.palette.base);
    if state.confirm.is_some() {
        draw_confirm(f, state, area);
    } else if let Some(o) = state.overlay {
        match o {
            Overlay::Help => draw_help(f, state, area),
            Overlay::Activity => draw_activity(f, state, area),
            Overlay::Settings => draw_settings(f, state, area),
        }
    }
}

fn dim_backdrop(f: &mut Frame, area: Rect, base: ratatui::style::Color) {
    let buf = f.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            let fg = if matches!(cell.fg, ratatui::style::Color::Rgb(..)) {
                crate::theme_map::blend(cell.fg, base, 0.60)
            } else {
                base
            };
            let bg = if matches!(cell.bg, ratatui::style::Color::Rgb(..)) {
                crate::theme_map::blend(cell.bg, base, 0.60)
            } else {
                base
            };
            cell.set_fg(fg);
            cell.set_bg(bg);
        }
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn popup(state: &AppState, title: &str, _area: Rect) -> Block<'static> {
    let p = state.palette;
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.accent))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(p.base))
}

fn draw_confirm(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let Some(pending) = state.confirm.clone() else {
        return;
    };

    // Size the box from the wrapped detail so the action row can never be clipped.
    let width = 78.min(area.width.saturating_sub(4)).max(40);
    let inner_w = width.saturating_sub(4).max(20) as usize;
    let detail_lines: u16 = pending
        .detail
        .lines()
        .map(|l| l.chars().count().div_ceil(inner_w).max(1) as u16)
        .sum::<u16>()
        .max(1);
    let height = (8 + detail_lines).min(area.height.saturating_sub(2)).max(8);
    let box_area = centered(area, width, height);
    f.render_widget(Clear, box_area);

    let title = match pending.kind {
        PendingKind::KillGroup => "Confirm kill",
        _ => "Confirm privileged action",
    };
    let block = popup(state, title, box_area).padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(box_area);
    f.render_widget(block, box_area);

    let verb = match pending.kind {
        PendingKind::KillGroup => "kill now",
        PendingKind::FlushSwap => "flush swap",
        PendingKind::DropCaches | PendingKind::DropCachesAll => "drop caches",
        PendingKind::Compact => "compact",
        PendingKind::EnablePower => "apply udev rule",
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "⚠ ",
                Style::default().fg(p.crit).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                pending.label.clone(),
                Style::default().fg(p.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            pending.detail.clone(),
            Style::default().fg(p.dim),
        )),
        Line::from(""),
    ];
    if pending.pids.is_empty() {
        lines.push(Line::from(Span::styled(
            "Runs via pkexec; you will be asked to authenticate.",
            Style::default().fg(p.warn),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("targets {} process(es)", pending.pids.len()),
            Style::default().fg(p.dim),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " y ",
            Style::default()
                .fg(p.base)
                .bg(p.crit)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {verb}    "), Style::default().fg(p.text)),
        Span::styled(" n / Esc ", Style::default().fg(p.text).bg(p.border)),
        Span::styled(" cancel", Style::default().fg(p.dim)),
    ]));

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn draw_help(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let box_area = centered(area, 60, 20);
    f.render_widget(Clear, box_area);
    f.render_widget(popup(state, "Keybinds", box_area), box_area);
    let inner = popup(state, "Keybinds", box_area).inner(box_area);
    let keys = [
        ("1 / 2 / 3 / 4", "Overview / Processes / Memory / Power"),
        ("Tab", "next view"),
        ("/", "search"),
        ("j / k", "move selection"),
        ("Enter", "expand group"),
        ("x", "kill selected group"),
        ("k / t", "kill / terminate group"),
        ("z / u", "suspend / resume"),
        ("+ / -", "renice selected"),
        ("o", "open exe location"),
        ("p", "pin group"),
        ("g / f / t", "group / flat / tree"),
        ("c / C", "drop caches / drop all"),
        ("m", "compact memory"),
        ("w", "flush swap"),
        ("s", "settings"),
        ("a", "activity log"),
        ("r", "reload config"),
        ("q", "quit"),
    ];
    let lines: Vec<Line> = keys
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(format!(" {k:<14}"), Style::default().fg(p.accent)),
                Span::styled(*d, Style::default().fg(p.text)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_activity(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let box_area = centered(
        area,
        area.width.saturating_sub(8),
        area.height.saturating_sub(4),
    );
    f.render_widget(Clear, box_area);
    f.render_widget(popup(state, "Activity", box_area), box_area);
    let inner = popup(state, "Activity", box_area).inner(box_area);
    let mut lines = Vec::new();
    if state.activity.is_empty() {
        lines.push(Line::from(Span::styled(
            " No actions yet. Privileged actions always appear here.",
            Style::default().fg(p.dim),
        )));
    }
    for a in state.activity.iter().rev().take(inner.height as usize) {
        let (mark, color) = if !a.ok {
            ("✗", p.crit)
        } else if a.privileged {
            ("◆", p.warn)
        } else {
            ("✓", p.ok)
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {mark} "), Style::default().fg(color)),
            Span::styled(format!("{:<20}", a.action), Style::default().fg(p.accent)),
            Span::styled(
                format!("{}: {}", a.target, a.message),
                Style::default().fg(p.text),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_settings(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let width = 78.min(area.width.saturating_sub(4)).max(50);
    let height = (SETTINGS.len() as u16 + 6)
        .min(area.height.saturating_sub(2))
        .max(12);
    let box_area = centered(area, width, height);
    f.render_widget(Clear, box_area);
    let block = popup(state, "Settings", box_area).padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(box_area);
    f.render_widget(block, box_area);

    let list_h = inner.height.saturating_sub(3).max(1);
    let cursor = state.settings_cursor.min(SETTINGS.len() - 1);
    let offset = cursor.saturating_sub(list_h as usize - 1);
    let mut lines = Vec::new();
    for (i, setting) in SETTINGS
        .iter()
        .enumerate()
        .skip(offset)
        .take(list_h as usize)
    {
        let (label, value, hint) = setting_row(state, *setting);
        let selected = i == cursor;
        let mut spans = vec![
            Span::styled(format!(" {label:<24}"), Style::default().fg(p.text)),
            Span::styled(
                format!("{value:<20}"),
                Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(hint, Style::default().fg(p.dim)),
        ];
        if selected {
            for s in &mut spans {
                s.style = s.style.bg(p.surface);
            }
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), inner);

    let foot = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "j/k move · ←/→ change · enter toggle/run · S save · esc close",
                Style::default().fg(p.dim),
            )),
            Line::from(Span::styled(
                format!("{}", pulsar_core::config::Config::config_path().display()),
                Style::default().fg(p.dim),
            )),
        ])
        .wrap(Wrap { trim: true }),
        foot,
    );
}

fn setting_row(state: &AppState, setting: Setting) -> (&'static str, String, &'static str) {
    let c = &state.config;
    let onoff = |b: bool| {
        if b {
            "on".to_string()
        } else {
            "off".to_string()
        }
    };
    match setting {
        Setting::Theme => ("Theme", c.theme.clone(), "cycle"),
        Setting::Icons => ("Nerd icons", onoff(c.nerd_icons), "enter"),
        Setting::Refresh => ("Refresh", format!("{} ms", c.refresh_ms), "250 ms · live"),
        Setting::Smoothing => ("Smoothing", format!("{}%", c.smoothing), "10% steps"),
        Setting::Tdp => (
            "TDP override",
            c.tdp_w
                .map(|t| format!("{t:.0} W"))
                .unwrap_or_else(|| "auto".to_string()),
            "5 W · live",
        ),
        Setting::History => ("History", format!("{} samples", c.history_len), "restart"),
        Setting::HideKernel => ("Hide kernel threads", onoff(c.hide_kernel_threads), "enter"),
        Setting::Confirm => ("Confirm destructive", onoff(c.confirm_kill), "enter"),
        Setting::Save => ("Save config", String::new(), "enter"),
        Setting::Reload => ("Reload from disk", String::new(), "enter"),
        Setting::WriteTheme => ("Export theme", String::new(), "enter"),
        Setting::Reset => ("Reset defaults", String::new(), "enter"),
    }
}
