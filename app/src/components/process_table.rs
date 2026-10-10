//! The grouped process table: Windows-Task-Manager folding, sorting, filtering and controls.

use dioxus::prelude::*;
use pulsar_core::group::{Group, ProcSample};

use crate::format::{human_bytes, human_rate, human_watts, monogram, pct};
use crate::icon;
use crate::state::{AppState, Pending, PendingKind, SortKey, ViewMode};
use crate::components::widgets::Sparkline;

#[component]
pub fn ProcessesView() -> Element {
    let state = use_context::<AppState>();
    rsx! {
        Toolbar {}
        ActionBar {}
        ProcessTable {}
        div { class: "dim", style: "text-align:center; font-size:0.8em; padding: 6px;",
            "PSS is shown by default so group sums reconcile with the header. Hover a row for RSS/USS/Swap."
        }
    }
}

#[component]
fn Toolbar() -> Element {
    let state = use_context::<AppState>();
    let mode = *state.view_mode.read();
    let hide_kernel = *state.hide_kernel.read();
    let search_open = *state.search_open.read();
    let count = state.snapshot.read().groups.len();
    rsx! {
        div { class: "row panel", style: "gap: 12px;",
            if search_open {
                input {
                    r#type: "text",
                    placeholder: "search processes…",
                    value: "{state.search.read()}",
                    autofocus: true,
                    style: "width: 260px;",
                    oninput: move |e| {
                        let mut s = state.search;
                        s.set(e.value());
                    },
                }
            } else {
                button {
                    class: "btn",
                    onclick: move |_| {
                        let mut s = state.search_open;
                        s.set(true);
                    },
                    "search  /"
                }
            }
            div { class: "seg-toggle",
                for m in [ViewMode::Group, ViewMode::Flat, ViewMode::Tree] {
                    div {
                        class: if m == mode { "opt active" } else { "opt" },
                        onclick: move |_| {
                            let mut v = state.view_mode;
                            v.set(m);
                        },
                        "{m.label()}"
                    }
                }
            }
            label { class: "row", style: "gap: 6px; cursor: pointer;",
                input {
                    r#type: "checkbox",
                    checked: hide_kernel,
                    onchange: move |_| {
                        let mut h = state.hide_kernel;
                        let v = *h.read();
                        h.set(!v);
                    },
                }
                span { class: "dim", "hide kernel threads" }
            }
            div { class: "spacer" }
            span { class: "dim", "{count} groups" }
        }
    }
}

#[component]
fn ActionBar() -> Element {
    let state = use_context::<AppState>();
    let selected = state.selected.read().clone();
    let Some(key) = selected else {
        return rsx! {};
    };
    let Some(group) = state
        .snapshot
        .read()
        .groups
        .iter()
        .find(|g| g.key == key)
        .cloned()
    else {
        return rsx! {};
    };
    let pids = group.pids.clone();
    let first = group.pids.first().copied().unwrap_or(0);
    let group_name = group.name.clone();
    let group_label = group.label();
    let exe = state
        .snapshot
        .read()
        .process_by_pid(first)
        .and_then(|p| p.exe.clone());

    let pids_suspend = pids.clone();
    let pids_resume = pids.clone();
    let pids_kill = pids.clone();
    let pids_kill_force = pids.clone();
    let pids_term = pids.clone();
    let exe_open = exe.clone();
    let label_kill = group_label.clone();
    let label_force = group_label.clone();
    let cmdline = state
        .snapshot
        .read()
        .process_by_pid(first)
        .and_then(|p| p.cmdline.clone())
        .unwrap_or_default();

    rsx! {
        div { class: "row panel", style: "gap: 8px;",
            span { class: "monogram", "{monogram(&group_name)}" }
            strong { "{group_label}" }
            span { class: "dim", "{pids.len()} pid(s)" }
            div { class: "spacer" }
            button {
                class: "btn danger",
                onclick: move |_| {
                    let mut c = state.confirm;
                    c.set(Some(Pending {
                        kind: PendingKind::KillGroup,
                        label: label_kill.clone(),
                        detail: "SIGTERM, then SIGKILL after 3s for survivors.".into(),
                        pids: pids_kill.clone(),
                    }));
                },
                "kill group  x"
            }
            button {
                class: "btn",
                onclick: move |_| {
                    state.log(pulsar_core::actions::terminate(&pids_term));
                },
                "terminate"
            }
            button {
                class: "btn",
                onclick: move |_| {
                    state.log(pulsar_core::actions::suspend(&pids_suspend));
                },
                "suspend"
            }
            button {
                class: "btn",
                onclick: move |_| {
                    state.log(pulsar_core::actions::resume(&pids_resume));
                },
                "resume"
            }
            input {
                r#type: "range",
                min: "-20", max: "19", value: "0",
                title: "renice",
                onchange: move |e| {
                    if let Ok(n) = e.value().parse::<i32>() {
                        state.log(pulsar_core::actions::renice(first, n));
                    }
                },
            }
            button {
                class: "btn",
                onclick: move |_| {
                    let mut c = state.confirm;
                    c.set(Some(Pending {
                        kind: PendingKind::KillGroup,
                        label: label_force.clone(),
                        detail: "Force SIGKILL immediately.".into(),
                        pids: pids_kill_force.clone(),
                    }));
                },
                "force kill"
            }
            if let Some(exe) = exe_open {
                button {
                    class: "btn",
                    onclick: move |_| {
                        state.log(pulsar_core::actions::open_exe_location(&exe));
                    },
                    "open location"
                }
            }
            button {
                class: "btn",
                onclick: move |_| {
                    state.log(pulsar_core::actions::ActionResult {
                        action: "copy".into(),
                        target: first.to_string(),
                        ok: true,
                        message: format!("pid {first}: {cmdline}"),
                        privileged: false,
                    });
                },
                "copy pid"
            }
        }
    }
}

#[component]
fn ProcessTable() -> Element {
    let state = use_context::<AppState>();
    let snapshot = state.snapshot.read();
    let mode = *state.view_mode.read();
    let sort = *state.sort.read();
    let desc = *state.sort_desc.read();
    let order = state.order_for(&snapshot.groups);

    let header = |key: SortKey, label: &'static str| -> Element {
        let sorted = sort == key;
        let arrow = if sorted { if desc { " ▾" } else { " ▴" } } else { "" };
        rsx! {
            div {
                class: if sorted { "th sorted" } else { "th" },
                onclick: move |_| {
                    if *state.sort.read() == key {
                        let mut d = state.sort_desc;
                        let v = *d.read();
                        d.set(!v);
                    } else {
                        let mut s = state.sort;
                        s.set(key);
                        let mut d = state.sort_desc;
                        d.set(true);
                    }
                    state.force_reorder();
                },
                "{label}{arrow}"
            }
        }
    };

    // Build rows: group mode folds children under expanded groups; flat mode lists processes.
    let mut rows: Vec<Element> = Vec::new();
    if mode == ViewMode::Flat {
        let mut procs: Vec<&ProcSample> = snapshot.processes.iter().collect();
        procs.sort_by(|a, b| {
            let ord = match sort {
                SortKey::Cpu => a.cpu_pct.partial_cmp(&b.cpu_pct).unwrap_or(std::cmp::Ordering::Equal),
                SortKey::Power => a.power_w.partial_cmp(&b.power_w).unwrap_or(std::cmp::Ordering::Equal),
                SortKey::Io => (a.io.read_bytes + a.io.write_bytes).cmp(&(b.io.read_bytes + b.io.write_bytes)),
                SortKey::Rss => a.mem.rss.cmp(&b.mem.rss),
                SortKey::Pid => a.pid.cmp(&b.pid),
                _ => a.mem.pss.cmp(&b.mem.pss),
            };
            if desc { ord.reverse() } else { ord }
        });
        for p in procs.iter().take(400) {
            rows.push(rsx! { ProcessRow { key: "{p.pid}", p: (*p).clone() } });
        }
    } else {
        let visible: Vec<Group> = order
            .iter()
            .filter_map(|k| snapshot.groups.iter().find(|g| &g.key == k).cloned())
            .filter(|g| state.passes_filter(g, &snapshot))
            .collect();
        for g in &visible {
            let expanded = mode == ViewMode::Tree || state.expanded.read().contains(&g.key);
            let pinned = state.pinned.read().contains(&g.key);
            rows.push(rsx! {
                GroupRow { group: g.clone(), expanded, pinned }
            });
            if expanded {
                for pid in &g.pids {
                    if let Some(p) = snapshot.process_by_pid(*pid) {
                        rows.push(rsx! { ChildRow { p: p.clone() } });
                    }
                }
            }
        }
    }

    rsx! {
        div { class: "table panel", style: "padding: 0;",
            div { class: "thead",
                div {}
                {header(SortKey::Name, "Name")}
                {header(SortKey::Threads, "N")}
                {header(SortKey::Cpu, "CPU")}
                {header(SortKey::Pss, "Memory")}
                {header(SortKey::Io, "Disk")}
                {header(SortKey::Power, "Power")}
                div { class: "th", "" }
                {header(SortKey::Pid, "PID")}
            }
            div { style: "overflow: auto; max-height: 62vh;",
                {rows.into_iter()}
            }
        }
    }
}

fn icon_cell(name: &str, icon_name: Option<&str>) -> Element {
    let src = icon_name.and_then(icon::resolve);
    rsx! {
        span { class: "monogram",
            if let Some(src) = src {
                img { src: "{src}", alt: "" }
            } else {
                "{monogram(name)}"
            }
        }
    }
}

#[component]
fn GroupRow(group: Group, expanded: bool, pinned: bool) -> Element {
    let state = use_context::<AppState>();
    let key = group.key.clone();
    let key_pin = key.clone();
    let pids = group.pids.clone();
    let key_click = key.clone();
    let is_selected = state.selected.read().as_deref() == Some(key.as_str());
    let title = format!(
        "{} · {} procs · RSS {} · PSS {} · USS {} · swap {}",
        group.name,
        group.count(),
        human_bytes(group.rss as f64 * 1024.0),
        human_bytes(group.pss as f64 * 1024.0),
        human_bytes(group.uss as f64 * 1024.0),
        human_bytes(group.swap as f64 * 1024.0),
    );
    let spark = group.spark.clone();
    let style = if is_selected {
        "background: var(--accent-soft);"
    } else {
        ""
    };
    let kind = group.kind;
    rsx! {
        div {
            class: if group.is_kernel { "trow group kernel" } else { "trow group" },
            style: "{style}",
            title: "{title}",
            onclick: move |_| {
                let mut sel = state.selected;
                sel.set(Some(key_click.clone()));
                state.toggle_expanded(&key_click);
                state.force_reorder();
            },
            div { class: "row", style: "gap:2px;",
                button {
                    class: "btn",
                    style: "padding:0 5px; height:18px;",
                    onclick: move |e| {
                        e.stop_propagation();
                        state.toggle_pin(&key_pin);
                    },
                    if pinned { "★" } else { "☆" }
                }
                span { class: "dim", style: "font-size:0.8em;", if expanded { "▾" } else { "▸" } }
            }
            div { class: "name",
                {icon_cell(&group.name, group.icon.as_deref())}
                span { "{group.name}" }
                if group.count() > 1 {
                    span { class: "tag", "×{group.count()}" }
                }
                if kind != pulsar_core::group::GroupKind::App {
                    span { class: "tag", "{match kind { pulsar_core::group::GroupKind::Exe => "exe", _ => "tree" }}" }
                }
            }
            div { class: "num dim", "{group.count()}" }
            div { class: "num", "{pct(group.cpu_pct)}" }
            div { class: "num", "{human_bytes(group.pss as f64 * 1024.0)}" }
            div { class: "num dim", "{human_rate((group.io_read + group.io_write) as f64)}" }
            div { class: "pwr",
                "{human_watts(group.power_w)}"
                if !pids.is_empty() {
                    span { class: "tag est", style: "margin-left:5px;", "est." }
                }
            }
            div {
                Sparkline { values: spark, width: 56.0, height: 18.0 }
            }
            div { class: "num dim", "{group.pids.first().copied().unwrap_or(0)}" }
        }
    }
}

#[component]
fn ChildRow(p: ProcSample) -> Element {
    let exe = p.exe.clone();
    let title = format!(
        "{}\nPID {} · {} threads · RSS {} · PSS {} · swap {}\n{}",
        p.comm,
        p.pid,
        p.threads,
        human_bytes(p.mem.rss as f64 * 1024.0),
        human_bytes(p.mem.pss as f64 * 1024.0),
        human_bytes(p.mem.swap as f64 * 1024.0),
        p.cmdline.clone().unwrap_or_else(|| exe.clone().unwrap_or_default()),
    );
    let engine = if p.mem.from_rollup { "PSS" } else { "RSS" };
    rsx! {
        div { class: "trow child", title: "{title}",
            div {}
            div { class: "name",
                span { class: "dim", "{p.comm}" }
                span { class: "tag", "{engine}" }
            }
            div { class: "num dim", "{p.threads}" }
            div { class: "num", "{pct(p.cpu_pct)}" }
            div { class: "num", "{human_bytes(p.mem.pss as f64 * 1024.0)}" }
            div { class: "num dim", "{human_rate((p.io.read_bytes + p.io.write_bytes) as f64)}" }
            div { class: "pwr", "{human_watts(p.power_w)}" }
            div {}
            div { class: "num dim", "{p.pid}" }
        }
    }
}

#[component]
fn ProcessRow(p: ProcSample) -> Element {
    let title = format!(
        "{}\nPID {} · RSS {} · PSS {}",
        p.comm,
        p.pid,
        human_bytes(p.mem.rss as f64 * 1024.0),
        human_bytes(p.mem.pss as f64 * 1024.0),
    );
    rsx! {
        div { class: "trow", title: "{title}",
            div {}
            div { class: "name", {icon_cell(&p.comm, p.exe.as_deref())} span { "{p.comm}" } }
            div { class: "num dim", "{p.threads}" }
            div { class: "num", "{pct(p.cpu_pct)}" }
            div { class: "num", "{human_bytes(p.mem.pss as f64 * 1024.0)}" }
            div { class: "num dim", "{human_rate((p.io.read_bytes + p.io.write_bytes) as f64)}" }
            div { class: "pwr", "{human_watts(p.power_w)}" }
            div {}
            div { class: "num dim", "{p.pid}" }
        }
    }
}
