//! Pulsar — TUI system monitor.

mod dump;
mod format;
mod graph;
mod overlays;
mod state;
mod theme_map;
mod treemap;
mod ui;

use std::time::Duration;

use pulsar_core::sampler::Sampler;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use state::{AppState, Cli, Overlay, Pending, PendingKind, SortKey, View, ViewMode, CLI};

fn main() {
    pulsar_core::theme::Theme::seed_user_themes();
    let cli = parse_cli(std::env::args().skip(1));
    let dump = cli.dump_dir.clone();
    let is_dump = dump.is_some();
    let png = cli.png;
    let _ = CLI.set(cli);

    let (rx, _handle) = {
        let mut scfg = AppState::new().config.clone();
        // In dump mode sample fast so the history graphs are full for the capture.
        if is_dump {
            scfg.refresh_ms = 100;
        }
        Sampler::spawn(scfg)
    };

    if let Some(dir) = dump {
        dump::run(dir, png, rx);
        return;
    }

    let mut terminal = ratatui::init();
    let mut state = AppState::new();
    let result = run(&mut terminal, &mut state, rx);
    ratatui::restore();
    if let Err(e) = result {
        eprintln!("pulsar: {e}");
        std::process::exit(1);
    }
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut rx: tokio::sync::watch::Receiver<pulsar_core::snapshot::SystemSnapshot>,
) -> std::io::Result<()> {
    if rx.has_changed().unwrap_or(false) {
        state.ingest(rx.borrow_and_update().clone());
    }
    loop {
        // Ingest the latest snapshot without blocking.
        if rx.has_changed().unwrap_or(false) {
            state.ingest(rx.borrow_and_update().clone());
            if CLI.get().map(|c| c.once).unwrap_or(false) {
                state.should_quit = true;
            }
        }
        terminal.draw(|f| ui::draw(f, state))?;
        if state.should_quit {
            break;
        }
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(k) => handle_key(state, k),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}

fn parse_cli(args: impl Iterator<Item = String>) -> Cli {
    let mut cli = Cli::default();
    let mut it = args.peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dump" => cli.dump_dir = it.next().map(std::path::PathBuf::from),
            "--png" => cli.png = true,
            "--once" => cli.once = true,
            "--theme" => cli.theme = it.next(),
            "--view" => {
                cli.view = it.next().and_then(|v| match v.as_str() {
                    "overview" => Some(View::Overview),
                    "processes" => Some(View::Processes),
                    "memory" => Some(View::Memory),
                    "power" => Some(View::Power),
                    "ports" => Some(View::Ports),
                    _ => None,
                })
            }
            "--help" | "-h" => {
                println!(
                    "pulsar [--theme NAME] [--view overview|processes|memory|power|ports] \
                     [--dump DIR] [--png] [--once]"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    cli
}

fn handle_key(state: &mut AppState, k: KeyEvent) {
    // Search input mode consumes most keys.
    if state.search_active {
        match k.code {
            KeyCode::Esc => {
                state.search_active = false;
                state.search.clear();
            }
            KeyCode::Enter => state.search_active = false,
            KeyCode::Backspace => {
                state.search.pop();
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                state.search.push(c);
            }
            _ => {}
        }
        return;
    }

    // Confirmation modal.
    if let Some(pending) = state.confirm.clone() {
        match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                confirm(state, &pending);
                state.confirm = None;
            }
            KeyCode::Char('n') | KeyCode::Esc => state.confirm = None,
            _ => {}
        }
        return;
    }

    // Overlays.
    if state.overlay == Some(Overlay::Settings) {
        settings_key(state, k);
        return;
    }
    if state.overlay.is_some() {
        if matches!(
            k.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('a') | KeyCode::Char('?')
        ) {
            state.overlay = None;
        }
        return;
    }

    // The Ports view has its own row selection; arrows/j/k must not move the group selection.
    if state.view == View::Ports {
        match k.code {
            KeyCode::Char('j') | KeyCode::Down => {
                state.move_port_selection(1);
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.move_port_selection(-1);
                return;
            }
            KeyCode::Char('g') => {
                state.ports_selected = 0;
                return;
            }
            KeyCode::Char('G') => {
                let count = state.port_row_count();
                state.ports_selected = count.saturating_sub(1);
                return;
            }
            KeyCode::Enter => {
                state.toggle_port_group();
                return;
            }
            KeyCode::Char('u') => {
                state.ports_grouped = !state.ports_grouped;
                state.ports_selected = 0;
                return;
            }
            KeyCode::Char('x') => {
                kill_selected_port(state);
                return;
            }
            _ => {}
        }
    }

    match k.code {
        KeyCode::Char('q') => state.should_quit = true,
        KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true
        }
        KeyCode::Esc => {
            state.selected = None;
            state.search.clear();
        }
        KeyCode::Tab => next_view(state),
        KeyCode::Char('1') => state.view = View::Overview,
        KeyCode::Char('2') => state.view = View::Processes,
        KeyCode::Char('3') => state.view = View::Memory,
        KeyCode::Char('4') => state.view = View::Power,
        KeyCode::Char('5') => state.view = View::Ports,
        KeyCode::Char('/') => {
            state.search_active = true;
            // Keep the Ports view; otherwise search means the process table.
            if state.view != View::Ports {
                state.view = View::Processes;
            }
        }
        KeyCode::Char('?') => state.overlay = Some(Overlay::Help),
        KeyCode::Char('a') => state.overlay = Some(Overlay::Activity),
        KeyCode::Char('s') => state.overlay = Some(Overlay::Settings),
        KeyCode::Char('r') => state.reload_config(),
        KeyCode::Char('g') => ready_new_mode(state, ViewMode::Group),
        KeyCode::Char('f') => ready_new_mode(state, ViewMode::Flat),
        KeyCode::Char('t') if state.view == View::Processes => {
            ready_new_mode(state, ViewMode::Tree)
        }
        KeyCode::Char('S') => cycle_sort(state, false),
        KeyCode::Char('R') => cycle_sort(state, true),
        KeyCode::Char('h') => {
            state.hide_kernel = !state.hide_kernel;
            state.force_reorder();
        }
        KeyCode::Char('j') | KeyCode::Down => move_selection(state, 1),
        KeyCode::Char('k') | KeyCode::Up => move_selection(state, -1),
        KeyCode::Enter => {
            if let Some(key) = state.selected.clone() {
                state.toggle_expanded(&key);
            }
        }
        _ => view_action_keys(state, k),
    }
}

fn view_action_keys(state: &mut AppState, k: KeyEvent) {
    match state.view {
        View::Processes => process_keys(state, k),
        View::Memory => memory_keys(state, k),
        _ => {}
    }
}

fn kill_selected_port(state: &mut AppState) {
    let (pids, label, detail) = if state.ports_grouped {
        let Some(g) = state.selected_port_group() else {
            state.set_message("no port group selected");
            return;
        };
        let pids = g.pids();
        if pids.is_empty() {
            state.set_message(
                "cannot kill: no owning process readable (permission or kernel socket)",
            );
            return;
        }
        let name = g.name.clone();
        let ports = g.ports(40);
        (
            pids,
            format!("{name} · {} socket(s)", g.count()),
            format!(
                "Send SIGTERM to {name}, which owns {} socket(s) on port(s) {ports}, then \
                 SIGKILL survivors after 3s.",
                g.count()
            ),
        )
    } else {
        let Some(s) = state.selected_port() else {
            state.set_message("no socket selected");
            return;
        };
        let (pid, comm, port, proto) = (s.pid, s.comm.clone(), s.port, s.proto.clone());
        let Some(pid) = pid else {
            state.set_message(
                "cannot kill: no owning process readable (permission or kernel socket)",
            );
            return;
        };
        let name = comm.unwrap_or_else(|| "?".to_string());
        (
            vec![pid],
            format!("{name} (pid {pid}) · {proto}:{port}"),
            format!(
                "Send SIGTERM to {name} (pid {pid}), which owns {proto} port {port}, then \
                 SIGKILL survivors after 3s."
            ),
        )
    };
    if state.config.confirm_kill {
        state.confirm = Some(Pending {
            kind: PendingKind::KillGroup,
            label,
            detail,
            pids,
        });
    } else {
        state.log(pulsar_core::actions::terminate_then_kill(
            pids,
            std::time::Duration::from_secs(3),
        ));
    }
}

fn process_keys(state: &mut AppState, k: KeyEvent) {
    let Some(g) = state.selected_group().cloned() else {
        return;
    };
    let pids = g.pids.clone();
    match k.code {
        KeyCode::Char('x') => {
            state.confirm = Some(Pending {
                kind: PendingKind::KillGroup,
                label: g.label(),
                detail: "SIGTERM now, then SIGKILL for any survivors after 3s.".into(),
                pids,
            });
        }
        KeyCode::Char('k') => {
            state.confirm = Some(Pending {
                kind: PendingKind::KillGroup,
                label: g.label(),
                detail: "SIGTERM now, then SIGKILL for any survivors after 3s.".into(),
                pids,
            });
        }
        KeyCode::Char('t') => state.log(pulsar_core::actions::terminate(&pids)),
        KeyCode::Char('z') => state.log(pulsar_core::actions::suspend(&pids)),
        KeyCode::Char('u') => state.log(pulsar_core::actions::resume(&pids)),
        KeyCode::Char('p') => state.toggle_pin(&g.key),
        KeyCode::Char('o') => {
            if let Some(exe) = first_exe(state, &g) {
                state.log(pulsar_core::actions::open_exe_location(&exe));
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            if let Some(pid) = g.pids.first() {
                state.log(pulsar_core::actions::renice(*pid, -5));
            }
        }
        KeyCode::Char('-') => {
            if let Some(pid) = g.pids.first() {
                state.log(pulsar_core::actions::renice(*pid, 5));
            }
        }
        _ => {}
    }
}

fn first_exe(state: &AppState, g: &pulsar_core::group::Group) -> Option<String> {
    g.pids
        .first()
        .and_then(|pid| state.snapshot.process_by_pid(*pid))
        .and_then(|p| p.exe.clone())
}

fn memory_keys(state: &mut AppState, k: KeyEvent) {
    match k.code {
        KeyCode::Char('c') => {
            state.confirm = Some(priviledged(PendingKind::DropCaches, "Drop clean caches",
                "Runs `sync; echo 1 > /proc/sys/vm/drop_caches` via pkexec. Frees only already-clean page cache; the kernel re-reads files, so IO may spike. Does not free tmpfs or swap."));
        }
        KeyCode::Char('C') => {
            state.confirm = Some(priviledged(PendingKind::DropCachesAll, "Drop caches + dentries + inodes",
                "Runs `sync; echo 3 > /proc/sys/vm/drop_caches`. More aggressive; directory traversal slows until caches rebuild."));
        }
        KeyCode::Char('m') => {
            state.confirm = Some(priviledged(
                PendingKind::Compact,
                "Compact memory",
                "Runs `echo 1 > /proc/sys/vm/compact_memory`. Defragments physical memory.",
            ));
        }
        KeyCode::Char('w') => {
            if pulsar_core::actions::swap_flush_blocker(&state.snapshot.mem).is_none() {
                state.confirm = Some(priviledged(PendingKind::FlushSwap, "Flush swap",
                    "Runs `swapoff -a && swapon -a` via pkexec. Reads every swapped page back into RAM; it will freeze briefly."));
            } else {
                state.set_message("flush swap disabled: not enough free RAM to swap back in");
            }
        }
        KeyCode::Char('p') if !state.snapshot.rapl_readable => {
            state.confirm = Some(priviledged(PendingKind::EnablePower, "Enable power metrics",
                "Writes a udev rule (99-pulsar-rapl.rules) making /sys/class/powercap/*/energy_uj readable by the wheel group, then reloads udev. Enables measured RAPL wattage."));
        }
        _ => {}
    }
}

fn priviledged(kind: PendingKind, label: &str, detail: &str) -> Pending {
    Pending {
        kind,
        label: label.to_string(),
        detail: detail.to_string(),
        pids: Vec::new(),
    }
}

fn confirm(state: &mut AppState, pending: &Pending) {
    match pending.kind {
        PendingKind::KillGroup => {
            let r = pulsar_core::actions::terminate_then_kill(
                pending.pids.clone(),
                std::time::Duration::from_secs(3),
            );
            state.log(r);
        }
        PendingKind::FlushSwap => state.log(pulsar_core::actions::flush_swap()),
        PendingKind::DropCaches => state.log(pulsar_core::actions::drop_caches()),
        PendingKind::DropCachesAll => state.log(pulsar_core::actions::drop_caches_all()),
        PendingKind::Compact => state.log(pulsar_core::actions::compact_memory()),
        PendingKind::EnablePower => state.log(pulsar_core::actions::enable_power_metrics()),
    }
}

fn settings_key(state: &mut AppState, k: KeyEvent) {
    match k.code {
        KeyCode::Esc | KeyCode::Char('s') => state.overlay = None,
        KeyCode::Char('t') => {
            let names = pulsar_core::theme::Theme::builtin_names();
            let idx = names
                .iter()
                .position(|n| *n == state.config.theme)
                .map(|i| (i + 1) % names.len())
                .unwrap_or(0);
            state.config.theme = names[idx].to_string();
            state.theme = pulsar_core::theme::Theme::resolve(&state.config.theme);
            state.palette = crate::theme_map::Palette::from_theme(&state.theme);
        }
        KeyCode::Char(']') => {
            state.config.refresh_ms = (state.config.refresh_ms + 100).min(5000);
        }
        KeyCode::Char('[') => {
            state.config.refresh_ms = state.config.refresh_ms.saturating_sub(100).max(500);
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            state.config.opacity = (state.config.opacity + 0.05).min(1.0);
        }
        KeyCode::Char('-') => {
            state.config.opacity = (state.config.opacity - 0.05).max(0.5);
        }
        KeyCode::Char('h') => {
            state.config.hide_kernel_threads = !state.config.hide_kernel_threads;
            state.hide_kernel = state.config.hide_kernel_threads;
        }
        KeyCode::Char('K') => state.config.confirm_kill = !state.config.confirm_kill,
        KeyCode::Char('d') => {
            state.config.density = match state.config.density {
                pulsar_core::config::Density::Compact => pulsar_core::config::Density::Comfy,
                pulsar_core::config::Density::Comfy => pulsar_core::config::Density::Compact,
            };
        }
        KeyCode::Char('S') => {
            let r = match state.config.save() {
                Ok(_) => pulsar_core::actions::ActionResult {
                    action: "save-config".into(),
                    target: "config.toml".into(),
                    ok: true,
                    message: format!(
                        "saved to {}",
                        pulsar_core::config::Config::config_path().display()
                    ),
                    privileged: false,
                },
                Err(e) => pulsar_core::actions::ActionResult {
                    action: "save-config".into(),
                    target: "config.toml".into(),
                    ok: false,
                    message: format!("{e}"),
                    privileged: false,
                },
            };
            state.log(r);
        }
        KeyCode::Char('w') => {
            let dir = pulsar_core::theme::Theme::theme_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(format!("{}.toml", state.config.theme));
            let r = match std::fs::write(&path, state.theme.to_toml()) {
                Ok(_) => pulsar_core::actions::ActionResult {
                    action: "write-theme".into(),
                    target: path.display().to_string(),
                    ok: true,
                    message: "written".into(),
                    privileged: false,
                },
                Err(e) => pulsar_core::actions::ActionResult {
                    action: "write-theme".into(),
                    target: path.display().to_string(),
                    ok: false,
                    message: format!("{e}"),
                    privileged: false,
                },
            };
            state.log(r);
        }
        _ => {}
    }
}

fn next_view(state: &mut AppState) {
    let all = View::all();
    let idx = all.iter().position(|v| *v == state.view).unwrap_or(0);
    state.view = all[(idx + 1) % all.len()];
}

fn ready_new_mode(state: &mut AppState, mode: ViewMode) {
    state.view_mode = mode;
    state.force_reorder();
}

fn cycle_sort(state: &mut AppState, reverse: bool) {
    if reverse {
        state.sort_desc = !state.sort_desc;
        state.force_reorder();
        return;
    }
    let all = SortKey::all();
    let idx = all.iter().position(|k| *k == state.sort).unwrap_or(0);
    state.sort = all[(idx + 1) % all.len()];
    state.force_reorder();
}

fn move_selection(state: &mut AppState, delta: i32) {
    let keys = state.visible_keys();
    if keys.is_empty() {
        return;
    }
    let idx = state
        .selected
        .as_ref()
        .and_then(|k| keys.iter().position(|x| x == k))
        .map(|i| i as i32)
        .unwrap_or(-1);
    let next = ((idx + delta).rem_euclid(keys.len() as i32)) as usize;
    state.selected = Some(keys[next].clone());
}
