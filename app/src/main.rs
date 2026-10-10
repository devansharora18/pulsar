mod components;
mod format;
mod icon;
mod state;
mod theme_css;
mod treemap;

use dioxus::prelude::*;
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use pulsar_core::sampler::Sampler;
use std::path::PathBuf;

use components::overlays::Overlays;
use components::sidebar::Sidebar;
use components::titlebar::Titlebar;
use components::views::Content;
use state::{AppState, Cli, Pending, PendingKind, View, ViewMode, CLI};

fn main() {
    pulsar_core::theme::Theme::seed_user_themes();
    let cli = parse_cli(std::env::args().skip(1));
    let screenshot = cli.screenshot.clone();
    let delay = cli.delay_secs.max(3);
    let _ = CLI.set(cli);

    if let Some(path) = screenshot {
        spawn_capture(path, delay);
    }

    let cfg = Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title("Pulsar")
                .with_decorations(false)
                .with_transparent(true)
                .with_inner_size(LogicalSize::new(1180.0, 780.0))
                .with_min_inner_size(LogicalSize::new(820.0, 520.0)),
        )
        .with_background_color((0, 0, 0, 0));
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(App);
}

fn parse_cli(args: impl Iterator<Item = String>) -> Cli {
    let mut cli = Cli { delay_secs: 4, ..Default::default() };
    let mut it = args.peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--screenshot" => cli.screenshot = it.next().map(PathBuf::from),
            "--theme" => cli.theme = it.next(),
            "--view" => {
                cli.view = it.next().and_then(|v| match v.as_str() {
                    "overview" => Some(View::Overview),
                    "processes" => Some(View::Processes),
                    "memory" => Some(View::Memory),
                    "power" => Some(View::Power),
                    _ => None,
                })
            }
            "--expand" => cli.expand = true,
            "--delay" => cli.delay_secs = it.next().and_then(|v| v.parse().ok()).unwrap_or(4),
            _ => {}
        }
    }
    cli
}

/// In `--screenshot` mode, wait for the UI to populate, capture the screen, then exit.
fn spawn_capture(path: PathBuf, delay_secs: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(delay_secs));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let on_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
        let status = if on_wayland {
            std::process::Command::new("grim").arg(&path).status()
        } else {
            std::process::Command::new("import").args(["-window", "root"]).arg(&path).status()
        };
        match status {
            Ok(s) if s.success() => eprintln!("pulsar: captured {path:?}"),
            other => eprintln!("pulsar: capture failed ({other:?}); install grim (wayland) or imagemagick (x11)"),
        }
        std::process::exit(0);
    });
}

#[component]
fn App() -> Element {
    let state = use_context_provider(AppState::new);
    let rx = use_hook(|| Sampler::spawn(state.config_snapshot()).0);

    // Pump snapshots from the sampler thread into the Dioxus signal.
    use_future(move || {
        let mut rx = rx.clone();
        async move {
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                let snap = rx.borrow_and_update().clone();
                let mut s = state.snapshot;
                s.set(snap);
                // In screenshot mode, reveal a few groups so the capture shows expansion.
                if CLI.get().map(|c| c.expand).unwrap_or(false) && state.expanded.read().is_empty() {
                    let snap = state.snapshot.read();
                    let keys: Vec<String> =
                        snap.groups.iter().take(4).map(|g| g.key.clone()).collect();
                    drop(snap);
                    let mut e = state.expanded;
                    let mut set = e.write();
                    for k in keys {
                        set.insert(k);
                    }
                }
            }
        }
    });

    // Hot-reload config/theme on file change.
    use_future(move || async move {
        let mut last: Option<std::time::SystemTime> = None;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let path = pulsar_core::config::Config::config_path();
            let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
            if mtime != last {
                last = mtime;
                state.reload_config();
            }
        }
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }
        style { dangerous_inner_html: "{state.css()}" }
        div {
            class: "window",
            tabindex: "0",
            onmounted: move |e| {
                let _ = e.set_focus(true);
            },
            onkeydown: move |e| handle_key(state, &e),
            Titlebar {}
            div { class: "body",
                Sidebar {}
                Content {}
            }
            Overlays {}
        }
    }
}

fn handle_key(state: AppState, e: &KeyboardData) {
    // Ignore keys while typing in the search box (except Escape).
    let typing = *state.search_open.read();
    let key = e.key();
    match key {
        Key::Escape => {
            let mut s = state.search_open;
            s.set(false);
            let mut c = state.cheat;
            c.set(false);
            let mut d = state.drawer;
            d.set(None);
            let mut set = state.settings;
            set.set(false);
        }
        Key::Character(ref c) if typing && c != "/" => {}
        Key::Character(ref c) => match c.as_str() {
            "1" => set_view(state, View::Overview),
            "2" => set_view(state, View::Processes),
            "3" => set_view(state, View::Memory),
            "4" => set_view(state, View::Power),
            "/" => {
                let mut s = state.search_open;
                s.set(true);
            }
            "?" => {
                let mut c = state.cheat;
                let v = *c.read();
                c.set(!v);
            }
            "s" => {
                let mut s = state.settings;
                let v = *s.read();
                s.set(!v);
            }
            "a" => {
                let mut d = state.drawer;
                d.set(Some(state::Drawer::Activity));
            }
            "r" => state.reload_config(),
            "g" => {
                let mut m = state.view_mode;
                m.set(ViewMode::Group);
            }
            "f" => {
                let mut m = state.view_mode;
                m.set(ViewMode::Flat);
            }
            "t" => {
                let mut m = state.view_mode;
                m.set(ViewMode::Tree);
            }
            "j" => move_selection(state, 1),
            "k" => move_selection(state, -1),
            "x" => kill_selected(state),
            _ => {}
        },
        Key::Enter => {
            if let Some(key) = state.selected.read().clone() {
                state.toggle_expanded(&key);
            }
        }
        _ => {}
    }
}

fn set_view(state: AppState, v: View) {
    let mut view = state.view;
    view.set(v);
}

fn ordered_keys(state: AppState) -> Vec<String> {
    let snapshot = state.snapshot.read();
    state.order_for(&snapshot.groups)
}

fn move_selection(state: AppState, delta: i32) {
    let keys = ordered_keys(state);
    if keys.is_empty() {
        return;
    }
    let current = state.selected.read().clone();
    let idx = current
        .and_then(|k| keys.iter().position(|x| *x == k))
        .map(|i| i as i32)
        .unwrap_or(-1);
    let next = ((idx + delta).rem_euclid(keys.len() as i32)) as usize;
    let mut sel = state.selected;
    sel.set(Some(keys[next].clone()));
}

fn kill_selected(state: AppState) {
    let Some(key) = state.selected.read().clone() else {
        return;
    };
    let snapshot = state.snapshot.read();
    let group = snapshot.groups.iter().find(|g| g.key == key);
    if let Some(g) = group {
        let mut c = state.confirm;
        c.set(Some(Pending {
            kind: PendingKind::KillGroup,
            label: g.label(),
            detail: "SIGTERM, then SIGKILL after 3s for survivors.".into(),
            pids: g.pids.clone(),
        }));
    }
}
