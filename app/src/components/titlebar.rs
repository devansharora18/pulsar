//! Custom frameless titlebar: drag, minimize, maximize, close.

use dioxus::desktop::use_window;
use dioxus::prelude::*;

use crate::format::{human_bytes, human_watts, pct};
use crate::state::{AppState, Drawer};

#[component]
pub fn Titlebar() -> Element {
    let state = use_context::<AppState>();
    let win = use_window();
    let snap = state.snapshot.read();
    let mem_pct = snap.mem.used_fraction() * 100.0;
    let power = snap.power.total_w;
    let power_label = match snap.power.kind {
        pulsar_core::power::PowerKind::Rapl => "est.",
        pulsar_core::power::PowerKind::Model => "model",
        pulsar_core::power::PowerKind::Unavailable => "",
    };
    drop(snap);

    let win_drag = win.clone();
    let win_min = win.clone();
    let win_max = win.clone();
    let win_close = win.clone();

    rsx! {
        div {
            class: "titlebar",
            onmousedown: move |_| win_drag.drag(),
            div { class: "brand", "PULSAR" }
            div { class: "sub", "· {human_bytes(state.snapshot.read().mem.used_real as f64 * 1024.0)} used ({pct(mem_pct)})"
            }
            if power > 0.2 {
                div { class: "sub",
                    "· {human_watts(power)}"
                    if !power_label.is_empty() {
                        span { class: "tag est", style: "margin-left:6px;", "{power_label}" }
                    }
                }
            }
            div { class: "spacer" }
            button {
                class: "win-btn",
                title: "Activity log",
                onclick: move |_| {
                    let mut d = state.drawer;
                    d.set(Some(Drawer::Activity));
                },
                "☰"
            }
            button {
                class: "win-btn",
                title: "Settings",
                onclick: move |_| {
                    let mut s = state.settings;
                    s.set(true);
                },
                "⚙"
            }
            div { class: "win-buttons",
                button { class: "win-btn", onclick: move |_| win_min.set_minimized(true), "—" }
                button { class: "win-btn", onclick: move |_| win_max.toggle_maximized(), "▢" }
                button { class: "win-btn close", onclick: move |_| win_close.close(), "✕" }
            }
        }
    }
}
