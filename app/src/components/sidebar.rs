//! Left navigation rail.

use dioxus::prelude::*;

use crate::state::{AppState, View};

#[component]
pub fn Sidebar() -> Element {
    let state = use_context::<AppState>();
    let current = *state.view.read();
    let snapshot = state.snapshot.read();
    let proc_count = snapshot.proc_count;
    let group_count = snapshot.groups.len();
    drop(snapshot);

    let view_list = View::all();
    rsx! {
        div { class: "sidebar",
            for (i, v) in view_list.into_iter().enumerate() {
                div {
                    class: if v == current { "nav-item active" } else { "nav-item" },
                    onclick: move |_| {
                        let mut view = state.view;
                        view.set(v);
                    },
                    span { "{v.icon()}" }
                    span { "{v.label()}" }
                    span { class: "kbd", "{i + 1}" }
                }
            }
            div { class: "spacer" }
            div { style: "font-size:0.8em; color: var(--fg-dim); padding: 6px;",
                div { "{group_count} groups" }
                div { "{proc_count} processes" }
            }
        }
    }
}
