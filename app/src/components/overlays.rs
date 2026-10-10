//! Settings drawer, activity log drawer, confirmation modal and the key cheat sheet.

use dioxus::prelude::*;
use pulsar_core::actions;
use pulsar_core::config::Density;
use pulsar_core::theme::Theme;

use crate::state::{AppState, Drawer, Pending, PendingKind};

#[component]
pub fn Overlays() -> Element {
    let state = use_context::<AppState>();
    let drawer = *state.drawer.read();
    let settings = *state.settings.read();
    let cheat = *state.cheat.read();
    let confirm = state.confirm.read().clone();
    rsx! {
        if settings || drawer == Some(Drawer::Settings) {
            SettingsDrawer {}
        }
        if drawer == Some(Drawer::Activity) {
            ActivityDrawer {}
        }
        if cheat {
            CheatSheet {}
        }
        if let Some(p) = confirm {
            ConfirmModal { pending: p }
        }
    }
}

#[component]
fn SettingsDrawer() -> Element {
    let state = use_context::<AppState>();
    let cfg = state.config_snapshot();
    let theme = state.theme_snapshot();
    rsx! {
        div {
            class: "overlay",
            onclick: move |_| {
                let mut s = state.settings;
                s.set(false);
                let mut d = state.drawer;
                d.set(None);
            },
            div {
                class: "drawer",
                onclick: move |e| e.stop_propagation(),
                div { class: "row",
                    strong { "Settings" }
                    div { class: "spacer" }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            let mut s = state.settings;
                            s.set(false);
                            let mut d = state.drawer;
                            d.set(None);
                        },
                        "close"
                    }
                }

                div { class: "field",
                    label { "Theme" }
                    select {
                        onchange: move |e| {
                            let name = e.value();
                            let mut t = state.theme;
                            t.set(Theme::resolve(&name));
                            let mut cfg = state.config;
                            let mut c = cfg.read().clone();
                            c.theme = name;
                            cfg.set(c);
                        },
                        for name in Theme::builtin_names() {
                            option {
                                value: "{name}",
                                selected: cfg.theme == *name,
                                "{name}"
                            }
                        }
                    }
                    div { class: "dim", style: "font-size:0.8em;", "User themes: {Theme::theme_dir().display()}/<name>.toml · pywal reads ~/.cache/wal/colors.json" }
                }

                div { class: "field",
                    label { "Refresh: {cfg.refresh_ms} ms" }
                    input {
                        r#type: "range", min: "500", max: "5000", step: "100", value: "{cfg.refresh_ms}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u64>() {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.refresh_ms = v.clamp(500, 5000);
                                cfg.set(c);
                            }
                        },
                    }
                }

                div { class: "field",
                    label { "Opacity: {cfg.opacity:.2}" }
                    input {
                        r#type: "range", min: "0.5", max: "1", step: "0.01", value: "{cfg.opacity}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f64>() {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.opacity = v.clamp(0.5, 1.0);
                                cfg.set(c);
                            }
                        },
                    }
                }

                div { class: "field",
                    label { "Corner radius: {cfg.corner_radius}px" }
                    input {
                        r#type: "range", min: "0", max: "32", value: "{cfg.corner_radius}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.corner_radius = v.min(48);
                                cfg.set(c);
                            }
                        },
                    }
                }

                div { class: "field",
                    label { "Font size: {cfg.font_size}px" }
                    input {
                        r#type: "range", min: "9", max: "24", value: "{cfg.font_size}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.font_size = v.clamp(9, 28);
                                cfg.set(c);
                            }
                        },
                    }
                }

                div { class: "field",
                    label { "Density" }
                    div { class: "seg-toggle",
                        for d in [Density::Compact, Density::Comfy] {
                            div {
                                class: if cfg.density == d { "opt active" } else { "opt" },
                                onclick: move |_| {
                                    let mut cfg = state.config;
                                    let mut c = cfg.read().clone();
                                    c.density = d;
                                    cfg.set(c);
                                },
                                if d == Density::Compact { "compact" } else { "comfy" }
                            }
                        }
                    }
                }

                div { class: "row",
                    label { class: "row", style: "gap:6px;",
                        input {
                            r#type: "checkbox", checked: cfg.blur,
                            onchange: move |_| {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.blur = !c.blur;
                                cfg.set(c);
                            },
                        }
                        "background blur (compositor hint)"
                    }
                }
                div { class: "row",
                    label { class: "row", style: "gap:6px;",
                        input {
                            r#type: "checkbox", checked: cfg.hide_kernel_threads,
                            onchange: move |_| {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.hide_kernel_threads = !c.hide_kernel_threads;
                                let hk = c.hide_kernel_threads;
                                cfg.set(c);
                                let mut h = state.hide_kernel;
                                h.set(hk);
                            },
                        }
                        "hide kernel threads by default"
                    }
                }
                div { class: "row",
                    label { class: "row", style: "gap:6px;",
                        input {
                            r#type: "checkbox", checked: cfg.confirm_kill,
                            onchange: move |_| {
                                let mut cfg = state.config;
                                let mut c = cfg.read().clone();
                                c.confirm_kill = !c.confirm_kill;
                                cfg.set(c);
                            },
                        }
                        "confirm before kill / privileged actions"
                    }
                }

                div { class: "row", style: "gap:8px;",
                    button {
                        class: "btn primary",
                        onclick: move |_| {
                            let cfg = state.config_snapshot();
                            state.log(actions::ActionResult {
                                action: "save-config".into(),
                                target: "config.toml".into(),
                                ok: cfg.save().is_ok(),
                                message: format!("saved to {}", pulsar_core::config::Config::config_path().display()),
                                privileged: false,
                            });
                        },
                        "save config"
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            state.reload_config();
                        },
                        "reload from disk"
                    }
                }
                div { class: "dim", style: "font-size:0.8em;", "mtimes are watched; edit the file and Pulsar reloads it live." }
                div { class: "dim", style: "font-size:0.8em;", "theme: {theme.name}" }
            }
        }
    }
}

#[component]
fn ActivityDrawer() -> Element {
    let state = use_context::<AppState>();
    let items = state.activity.read().clone();
    rsx! {
        div {
            class: "overlay",
            onclick: move |_| {
                let mut d = state.drawer;
                d.set(None);
            },
            div {
                class: "drawer",
                onclick: move |e| e.stop_propagation(),
                div { class: "row",
                    strong { "Activity" }
                    div { class: "spacer" }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            let mut a = state.activity;
                            a.set(Vec::new());
                        },
                        "clear"
                    }
                }
                if items.is_empty() {
                    div { class: "dim", "No actions yet. Privileged actions always appear here." }
                }
                for (i, a) in items.iter().enumerate().rev() {
                    div {
                        class: if !a.ok { "activity-item err" } else if a.privileged { "activity-item priv" } else { "activity-item ok" },
                        key: "{i}",
                        div { class: "row",
                            strong { "{a.action}" }
                            div { class: "spacer" }
                            if a.privileged { span { class: "tag model", "privileged" } }
                        }
                        div { class: "dim", "{a.target}: {a.message}" }
                    }
                }
            }
        }
    }
}

#[component]
fn CheatSheet() -> Element {
    let state = use_context::<AppState>();
    let keys = [
        ("1 / 2 / 3 / 4", "switch view"),
        ("/", "focus search"),
        ("j / k", "move selection down / up"),
        ("Enter", "expand / collapse group"),
        ("x", "kill selected group"),
        ("g / f / t", "group / flat / tree"),
        ("r", "reload config + theme"),
        ("s", "open settings"),
        ("a", "open activity"),
        ("?", "this cheat sheet"),
        ("Esc", "close overlays"),
    ];
    rsx! {
        div {
            class: "modal-wrap",
            onclick: move |_| {
                let mut c = state.cheat;
                c.set(false);
            },
            div {
                class: "modal",
                onclick: move |e| e.stop_propagation(),
                h3 { style: "margin-bottom:12px;", "Keybinds" }
                div { class: "cheat-grid",
                    for (k, d) in keys {
                        div { class: "row", span { kbd { "{k}" } } span { class: "dim", "{d}" } }
                    }
                }
            }
        }
    }
}

#[component]
fn ConfirmModal(pending: Pending) -> Element {
    let state = use_context::<AppState>();
    let p = pending.clone();
    let confirm_text = match pending.kind {
        PendingKind::KillGroup => "kill now",
        PendingKind::FlushSwap => "flush swap",
        PendingKind::DropCaches | PendingKind::DropCachesAll => "drop caches",
        PendingKind::Compact => "compact",
        PendingKind::EnablePower => "apply udev rule",
    };
    rsx! {
        div {
            class: "modal-wrap",
            onclick: move |_| {
                let mut c = state.confirm;
                c.set(None);
            },
            div {
                class: "modal",
                onclick: move |e| e.stop_propagation(),
                h3 { "{pending.label}" }
                p { class: "dim", style: "margin: 10px 0;", "{pending.detail}" }
                if !pending.pids.is_empty() {
                    div { class: "dim", style: "font-size:0.82em;", "pids: {pending.pids.len()} process(es)" }
                }
                div { class: "row", style: "gap:8px; margin-top:14px;",
                    button {
                        class: "btn danger",
                        onclick: move |_| {
                            dispatch(&state, &p);
                            let mut c = state.confirm;
                            c.set(None);
                        },
                        "{confirm_text}"
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            let mut c = state.confirm;
                            c.set(None);
                        },
                        "cancel"
                    }
                }
            }
        }
    }
}

fn dispatch(state: &AppState, p: &Pending) {
    match p.kind {
        PendingKind::KillGroup => {
            let r = actions::terminate_then_kill(p.pids.clone(), std::time::Duration::from_secs(3));
            state.log(r);
        }
        PendingKind::FlushSwap => state.log(actions::flush_swap()),
        PendingKind::DropCaches => state.log(actions::drop_caches()),
        PendingKind::DropCachesAll => state.log(actions::drop_caches_all()),
        PendingKind::Compact => state.log(actions::compact_memory()),
        PendingKind::EnablePower => state.log(actions::enable_power_metrics()),
    }
}
