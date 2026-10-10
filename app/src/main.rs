mod components;
mod format;
mod icon;
mod state;
mod theme_css;
mod treemap;

use dioxus::prelude::*;
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use pulsar_core::sampler::Sampler;

use components::overlays::Overlays;
use components::sidebar::Sidebar;
use components::titlebar::Titlebar;
use components::views::Content;
use state::{AppState, Pending, PendingKind, View, ViewMode};

fn main() {
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
