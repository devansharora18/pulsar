//! Popup overlays: confirmation modal, activity log, help and settings.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::state::{AppState, Overlay, PendingKind};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
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
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(p.accent))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(p.bg))
}

fn draw_confirm(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let Some(pending) = state.confirm.clone() else {
        return;
    };

    // Size the box from the wrapped detail so the action row can never be clipped.
    let width = 78.min(area.width.saturating_sub(4)).max(40);
    let inner_w = width.saturating_sub(4).max(20) as usize;
    let detail_lines = pending.detail.chars().count().div_ceil(inner_w).max(1) as u16;
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
                Style::default().fg(p.red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                pending.label.clone(),
                Style::default().fg(p.fg).add_modifier(Modifier::BOLD),
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
            Style::default().fg(p.orange),
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
                .fg(p.bg)
                .bg(p.red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {verb}    "), Style::default().fg(p.fg)),
        Span::styled(" n / Esc ", Style::default().fg(p.fg).bg(p.border)),
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
                Span::styled(*d, Style::default().fg(p.fg)),
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
            ("✗", p.red)
        } else if a.privileged {
            ("◆", p.orange)
        } else {
            ("✓", p.green)
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {mark} "), Style::default().fg(color)),
            Span::styled(format!("{:<20}", a.action), Style::default().fg(p.accent)),
            Span::styled(
                format!("{}: {}", a.target, a.message),
                Style::default().fg(p.fg),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_settings(f: &mut Frame, state: &AppState, area: Rect) {
    let p = state.palette;
    let box_area = centered(area, 66, 22);
    f.render_widget(Clear, box_area);
    f.render_widget(popup(state, "Settings", box_area), box_area);
    let inner = popup(state, "Settings", box_area).inner(box_area);
    let c = &state.config;
    let lines = vec![
        Line::from(Span::styled(
            " Adjust with the keys shown; changes apply live.",
            Style::default().fg(p.dim),
        )),
        Line::from(""),
        section(&state.palette, "Theme", &c.theme, "t cycles theme"),
        section(
            &state.palette,
            "Refresh",
            &format!("{} ms", c.refresh_ms),
            "[ ] adjusts 500–5000 ms",
        ),
        section(
            &state.palette,
            "Opacity",
            &format!("{:.2}", c.opacity),
            "- + adjust 0.5–1.0",
        ),
        section(
            &state.palette,
            "Font size",
            &format!("{}", c.font_size),
            "context only",
        ),
        section(
            &state.palette,
            "Corner radius",
            &format!("{}", c.corner_radius),
            "context only",
        ),
        section(
            &state.palette,
            "Density",
            &format!("{:?}", c.density).to_lowercase(),
            "d toggles compact/comfy",
        ),
        section(
            &state.palette,
            "Hide kernel threads",
            if c.hide_kernel_threads { "yes" } else { "no" },
            "h toggles",
        ),
        section(
            &state.palette,
            "Confirm privileged",
            if c.confirm_kill { "yes" } else { "no" },
            "K toggles",
        ),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " config: {}",
                pulsar_core::config::Config::config_path().display()
            ),
            Style::default().fg(p.dim),
        )),
        Line::from(Span::styled(
            format!(
                " themes: {}",
                pulsar_core::theme::Theme::theme_dir().display()
            ),
            Style::default().fg(p.dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [S] ", Style::default().fg(p.bg).bg(p.accent)),
            Span::styled("save   ", Style::default().fg(p.fg)),
            Span::styled(" [w] ", Style::default().fg(p.bg).bg(p.border)),
            Span::styled("write theme file   ", Style::default().fg(p.fg)),
            Span::styled(" [Esc] ", Style::default().fg(p.bg).bg(p.border)),
            Span::styled("close", Style::default().fg(p.fg)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn section<'a>(p: &crate::theme_map::Palette, label: &str, value: &str, hint: &str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!(" {label:<22}"), Style::default().fg(p.fg)),
        Span::styled(
            format!("{value:<16}"),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(hint.to_string(), Style::default().fg(p.dim)),
    ])
}
