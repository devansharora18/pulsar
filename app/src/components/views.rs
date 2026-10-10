//! The four main views: Overview, Processes, Memory and Power.

use dioxus::prelude::*;
use pulsar_core::power::PowerKind;

use crate::components::process_table::ProcessesView;
use crate::components::widgets::{Gauge, HeatGrid, Sparkline, StackBar};
use crate::format::{
    human_bytes, human_freq_mhz, human_kb, human_rate, human_temp, human_watts, pct,
};
use crate::state::{AppState, Pending, PendingKind, View};
use crate::treemap::squarify;

#[component]
pub fn Content() -> Element {
    let state = use_context::<AppState>();
    let view = *state.view.read();
    rsx! {
        div { class: "content",
            match view {
                View::Overview => rsx! { Overview {} },
                View::Processes => rsx! { ProcessesView {} },
                View::Memory => rsx! { MemoryView {} },
                View::Power => rsx! { PowerView {} },
            }
        }
    }
}

#[component]
fn Overview() -> Element {
    let state = use_context::<AppState>();
    let snap = state.snapshot.read();
    let mem = snap.mem.clone();
    let cpu = snap.cpu.clone();
    let power = snap.power;
    let series = snap.series.clone();
    let net = snap.net.clone();
    let disks = snap.disks.clone();
    let fss = snap.filesystems.clone();
    let temps = snap.temps.clone();
    let swap_frac = if mem.swap_total > 0 {
        mem.swap_used as f64 / mem.swap_total as f64 * 100.0
    } else {
        0.0
    };
    let cpu_temp = temps
        .iter()
        .find(|t| t.chip == "k10temp" || t.chip == "coretemp")
        .map(|t| t.temp_c);
    let gpu_temp = temps.iter().find(|t| t.chip == "amdgpu").map(|t| t.temp_c);
    let freqs: Vec<u64> = cpu.cores.iter().filter_map(|c| c.freq_mhz).collect();
    let avg_freq = if freqs.is_empty() {
        None
    } else {
        Some(freqs.iter().sum::<u64>() / freqs.len() as u64)
    };
    drop(snap);

    let power_label = match power.kind {
        PowerKind::Rapl => "est.",
        PowerKind::Model => "model",
        PowerKind::Unavailable => "",
    };

    rsx! {
        div { class: "grid cols-4",
            div { class: "panel", style: "display:grid; place-items:center;",
                Gauge {
                    value: cpu.overall_usage_pct, max: 100.0,
                    label: "{cpu.overall_usage_pct:.0}%", sub: "CPU",
                    color: "var(--accent)".to_string(), size: 116.0,
                }
            }
            div { class: "panel", style: "display:grid; place-items:center;",
                Gauge {
                    value: mem.used_real as f64, max: mem.total as f64,
                    label: "{human_bytes(mem.used_real as f64 * 1024.0)}",
                    sub: "of {human_bytes(mem.total as f64 * 1024.0)}",
                    color: "var(--magenta)".to_string(), size: 116.0,
                }
            }
            div { class: "panel", style: "display:grid; place-items:center;",
                Gauge {
                    value: mem.swap_used as f64, max: mem.swap_total.max(1) as f64,
                    label: "{human_bytes(mem.swap_used as f64 * 1024.0)}",
                    sub: "swap {swap_frac:.0}%",
                    color: "var(--yellow)".to_string(), size: 116.0,
                }
            }
            div { class: "panel", style: "display:grid; place-items:center;",
                Gauge {
                    value: power.total_w, max: 65.0,
                    label: "{human_watts(power.total_w)}", sub: "power",
                    color: "var(--green)".to_string(), size: 116.0,
                }
                if !power_label.is_empty() {
                    span { class: "tag est", "{power_label}" }
                }
            }
        }

        div { class: "grid cols-2",
            div { class: "panel",
                h3 { "CPU cores" }
                HeatGrid { cores: cpu.cores.clone() }
                div { class: "row", style: "margin-top:8px; font-size:0.86em;",
                    span { class: "dim", "load" }
                    span { "{cpu.load1:.2} {cpu.load5:.2} {cpu.load15:.2}" }
                    div { class: "spacer" }
                    if let Some(f) = avg_freq { span { class: "dim", "~{human_freq_mhz(f)}" } }
                    if let Some(t) = cpu_temp { span { "cpu {human_temp(t)}" } }
                    if let Some(t) = gpu_temp { span { "gpu {human_temp(t)}" } }
                }
                div { class: "dim", style: "font-size:0.78em; margin-top:4px;", "{cpu.model}" }
            }
            div { class: "panel",
                h3 { "Power" }
                Sparkline { values: series.power.clone(), width: 360.0, height: 52.0, color: "var(--green)".to_string() }
                div { class: "row", style: "margin-top:8px;",
                    span { "cpu {human_watts(power.pkg_w.unwrap_or(0.0))}" }
                    span { "gpu {human_watts(power.gpu_w.unwrap_or(0.0))}" }
                    if let Some(b) = power.battery_w {
                        span { "battery {human_watts(b)}" }
                    }
                }
            }
        }

        div { class: "grid cols-2",
            div { class: "panel",
                h3 { "Network" }
                for iface in net.iter().filter(|i| i.up || i.rx_bps > 0.0) {
                    div { class: "row", style: "margin-bottom:4px;",
                        span { style: "width:120px;", "{iface.name}" }
                        span { class: "dim", "↓ {human_rate(iface.rx_bps)}" }
                        span { class: "dim", "↑ {human_rate(iface.tx_bps)}" }
                    }
                }
                Sparkline { values: series.net_rx.clone(), width: 360.0, height: 40.0, color: "var(--blue)".to_string() }
                Sparkline { values: series.net_tx.clone(), width: 360.0, height: 40.0, color: "var(--cyan)".to_string() }
            }
            div { class: "panel",
                h3 { "Disk" }
                for d in disks.iter().filter(|d| d.read_bps > 0.0 || d.write_bps > 0.0).take(6) {
                    div { class: "row", style: "margin-bottom:4px;",
                        span { style: "width:120px;", "{d.name}" }
                        span { class: "dim", "r {human_rate(d.read_bps)}" }
                        span { class: "dim", "w {human_rate(d.write_bps)}" }
                    }
                }
                Sparkline { values: series.disk_read.clone(), width: 360.0, height: 40.0, color: "var(--orange)".to_string() }
            }
        }

        div { class: "panel",
            h3 { "Filesystems" }
            for fs in fss.iter() {
                div { class: "row", style: "gap:10px; margin-bottom:6px; font-size:0.86em;",
                    span { style: "width:180px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;", title: "{fs.device}",
                        "{fs.mount}"
                    }
                    span { class: "dim", style: "width:64px;", "{fs.fstype}" }
                    div { style: "flex:1; height:8px; background: var(--border); border-radius:4px; overflow:hidden;",
                        div {
                            style: "width: {fs.used_fraction() * 100.0:.1}%; height:100%; background: var(--accent);"
                        }
                    }
                    span { class: "mono-num", style: "width:150px; text-align:right;",
                        "{human_bytes(fs.used_bytes as f64)} / {human_bytes(fs.total_bytes as f64)}"
                    }
                }
            }
        }
    }
}

#[component]
fn MemoryView() -> Element {
    let state = use_context::<AppState>();
    let snap = state.snapshot.read();
    let mem = snap.mem.clone();
    let pressure = snap.pressure;
    let groups = snap.groups.clone();
    let sum_pss = snap.sum_pss;
    let rapl = snap.rapl_readable;
    drop(snap);

    let un = mem.unattributed(sum_pss);
    let psi = pressure.gauge();
    let swap_blocker = pulsar_core::actions::swap_flush_blocker(&mem);
    let swap_disabled = swap_blocker.is_some();
    let swap_title = swap_blocker.clone().unwrap_or_default();
    let swap_reason = swap_blocker.clone();

    // Plain-language summary line.
    let summary = format!(
        "{} really used · {} of cache you can reclaim · {} available",
        human_kb(mem.used_real),
        human_kb(mem.cache_reclaimable),
        human_kb(mem.available),
    );

    // Treemap data: top groups by PSS.
    let mut tm_groups: Vec<_> = groups.iter().filter(|g| g.pss > 0).collect();
    tm_groups.sort_by_key(|g| std::cmp::Reverse(g.pss));
    tm_groups.truncate(40);
    let values: Vec<f64> = tm_groups.iter().map(|g| g.pss as f64).collect();
    let rects = squarify(&values, 1000.0, 340.0);
    let palette = [
        "var(--accent)",
        "var(--blue)",
        "var(--magenta)",
        "var(--cyan)",
        "var(--green)",
        "var(--yellow)",
        "var(--orange)",
    ];

    rsx! {
        div { class: "panel",
            h3 { "Memory — one consistent story" }
            StackBar { mem: mem.clone(), sum_pss }
            div { style: "margin-top:10px; font-size:1.0em;", "{summary}" }
        }

        div { class: "grid cols-3",
            div { class: "panel",
                h3 { "Really used" }
                div { style: "font-size:1.6em; font-weight:700;", "{human_kb(mem.used_real)}" }
                div { class: "dim", "{pct(mem.used_fraction() * 100.0)} of total" }
                div { class: "row", style: "margin-top:8px; font-size:0.86em;",
                    span { "PSI memory" }
                    div { style: "flex:1; height:8px; background: var(--border); border-radius:4px; overflow:hidden;",
                        div { style: "width: {psi:.0}%; height:100%; background: var(--yellow);" }
                    }
                    span { "{psi:.1}" }
                }
            }
            div { class: "panel",
                h3 { "Reclaimable" }
                div { style: "font-size:1.6em; font-weight:700; color: var(--green);", "{human_kb(mem.cache_reclaimable)}" }
                div { class: "dim", "page cache + buffers + reclaimable slab" }
                div { class: "dim", style: "margin-top:6px;", "shared (tmpfs) {human_kb(mem.shared)}" }
            }
            div { class: "panel",
                h3 { "Kernel + unattributed" }
                div { style: "font-size:1.6em; font-weight:700; color: var(--orange);", "{human_kb(mem.kernel + un)}" }
                div { class: "dim", "kernel {human_kb(mem.kernel)} · unattributed {human_kb(un)}" }
                div { class: "dim", style: "margin-top:6px;", "sum of process PSS {human_kb(sum_pss)}" }
            }
        }

        div { class: "panel",
            h3 { "Memory treemap by app" }
            div { class: "treemap",
                for (i, g) in tm_groups.iter().enumerate() {
                    {
                        let r = rects.get(i).copied().unwrap_or_default();
                        let color = palette[i % palette.len()];
                        let key = g.key.clone();
                        let w = r.w / 1000.0 * 100.0;
                        let h = r.h / 340.0 * 100.0;
                        let x = r.x / 1000.0 * 100.0;
                        let y = r.y / 340.0 * 100.0;
                        let label = g.label();
                        let bytes = human_bytes(g.pss as f64 * 1024.0);
                        rsx! {
                            div {
                                class: "tm-cell",
                                style: "left:{x:.2}%; top:{y:.2}%; width:{w:.2}%; height:{h:.2}%; background: color-mix(in srgb, {color} 35%, var(--bg-alt)); border-left: 3px solid {color};",
                                title: "{label} · {bytes}",
                                onclick: move |_| {
                                    let mut sel = state.selected;
                                    sel.set(Some(key.clone()));
                                    let mut v = state.view;
                                    v.set(View::Processes);
                                },
                                div { class: "tm-label", "{label}" }
                                div { class: "tm-val", "{bytes}" }
                            }
                        }
                    }
                }
            }
        }

        div { class: "panel",
            h3 { "Reclaim memory" }
            div { class: "row", style: "gap: 10px; flex-wrap: wrap;",
                button {
                    class: "btn primary",
                    onclick: move |_| {
                        let mut c = state.confirm;
                        c.set(Some(Pending {
                            kind: PendingKind::DropCaches,
                            label: "Drop clean caches".into(),
                            detail: "Runs `sync; echo 1 > /proc/sys/vm/drop_caches` via pkexec. Frees only already-clean page cache; the kernel will re-read files, so IO may spike. Does not free tmpfs or swap.".into(),
                            pids: vec![],
                        }));
                    },
                    "drop caches (reclaimable)"
                }
                button {
                    class: "btn",
                    onclick: move |_| {
                        let mut c = state.confirm;
                        c.set(Some(Pending {
                            kind: PendingKind::DropCachesAll,
                            label: "Drop caches + dentries + inodes".into(),
                            detail: "Runs `sync; echo 3 > /proc/sys/vm/drop_caches`. More aggressive; directory traversal slows until caches rebuild.".into(),
                            pids: vec![],
                        }));
                    },
                    "drop all (3)"
                }
                button {
                    class: "btn",
                    disabled: swap_disabled,
                    title: "{swap_title}",
                    onclick: move |_| {
                        let mut c = state.confirm;
                        c.set(Some(Pending {
                            kind: PendingKind::FlushSwap,
                            label: "Flush swap".into(),
                            detail: "Runs `swapoff -a && swapon -a` via pkexec. Reads every swapped page back into RAM; it will freeze briefly and can OOM if RAM is tight.".into(),
                            pids: vec![],
                        }));
                    },
                    "flush swap"
                }
                button {
                    class: "btn",
                    onclick: move |_| {
                        let mut c = state.confirm;
                        c.set(Some(Pending {
                            kind: PendingKind::Compact,
                            label: "Compact memory".into(),
                            detail: "Runs `echo 1 > /proc/sys/vm/compact_memory`. Defragments physical memory; helps large contiguous allocations.".into(),
                            pids: vec![],
                        }));
                    },
                    "compact memory"
                }
                if !rapl {
                    button {
                        class: "btn warn",
                        onclick: move |_| {
                            let mut c = state.confirm;
                            c.set(Some(Pending {
                                kind: PendingKind::EnablePower,
                                label: "Enable power metrics".into(),
                                detail: "Writes a udev rule (99-pulsar-rapl.rules) making /sys/class/powercap/*/energy_uj readable by the `wheel` group, then reloads udev. This is what allows real RAPL wattage instead of the model.".into(),
                                pids: vec![],
                            }));
                        },
                        "enable power metrics (pkexec)"
                    }
                }
                if let Some(reason) = swap_reason {
                    span { class: "dim", "flush swap disabled: {reason}" }
                }
            }
        }
    }
}

#[component]
fn PowerView() -> Element {
    let state = use_context::<AppState>();
    let snap = state.snapshot.read();
    let power = snap.power;
    let series = snap.series.clone();
    let mut groups: Vec<_> = snap.groups.clone();
    drop(snap);
    groups.sort_by(|a, b| {
        b.power_w
            .partial_cmp(&a.power_w)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top: Vec<_> = groups.iter().filter(|g| g.power_w > 0.0).take(10).collect();
    let dt = state.config_snapshot().refresh_ms as f64 / 1000.0;
    let wh: f64 = series.power.iter().map(|p| p * dt / 3600.0).sum();
    let label = match power.kind {
        PowerKind::Rapl => "est. (RAPL)",
        PowerKind::Model => "model (util × TDP)",
        PowerKind::Unavailable => "unavailable",
    };

    rsx! {
        div { class: "grid cols-3",
            div { class: "panel", style: "display:grid; place-items:center;",
                Gauge { value: power.total_w, max: 65.0, label: "{human_watts(power.total_w)}", sub: "total", color: "var(--green)".to_string(), size: 132.0 }
                span { class: "tag est", "{label}" }
            }
            div { class: "panel",
                h3 { "Sources" }
                div { class: "row", style: "margin-bottom:4px;", span { class: "dim", style: "width:90px;", "CPU package" } span { "{human_watts(power.pkg_w.unwrap_or(0.0))}" } }
                div { class: "row", style: "margin-bottom:4px;", span { class: "dim", style: "width:90px;", "CPU dynamic" } span { "{human_watts(power.cpu_dynamic_w)}" } }
                div { class: "row", style: "margin-bottom:4px;", span { class: "dim", style: "width:90px;", "idle baseline" } span { "{human_watts(power.baseline_w)}" } }
                div { class: "row", style: "margin-bottom:4px;", span { class: "dim", style: "width:90px;", "GPU" } span { "{human_watts(power.gpu_w.unwrap_or(0.0))}" } }
                if let Some(b) = power.battery_w {
                    div { class: "row", span { class: "dim", style: "width:90px;", "battery" } span { "{human_watts(b)}" } }
                }
            }
            div { class: "panel",
                h3 { "Since launch" }
                div { style: "font-size:1.6em; font-weight:700;", "{wh:.3} Wh" }
                div { class: "dim", "accumulated energy estimate" }
            }
        }
        div { class: "panel",
            h3 { "System watts" }
            Sparkline { values: series.power.clone(), width: 760.0, height: 90.0, color: "var(--green)".to_string() }
        }
        div { class: "panel",
            h3 { "Top power consumers" }
            for g in top.iter() {
                div { class: "row", style: "gap:10px; margin-bottom:5px; font-size:0.88em;",
                    span { style: "width:200px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;", "{g.label()}" }
                    div { style: "flex:1; height:8px; background: var(--border); border-radius:4px; overflow:hidden;",
                        div { style: "width: {(g.power_w / 40.0 * 100.0).min(100.0):.1}%; height:100%; background: var(--orange);" }
                    }
                    span { class: "pwr", style: "width:70px; text-align:right;", "{human_watts(g.power_w)}" }
                    span { class: "tag est", "est." }
                }
            }
        }
    }
}
