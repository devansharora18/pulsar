//! Hand-built SVG widgets: gauges, sparklines, heat grid and the memory stack bar.

use dioxus::prelude::*;
use pulsar_core::cpu::CoreSample;
use pulsar_core::meminfo::MemoryBreakdown;

use crate::format::human_kb;

/// Build `(line, area)` SVG path strings for a sparkline.
pub fn spark_paths(values: &[f64], w: f64, h: f64, hi: f64) -> (String, String) {
    if values.is_empty() || w <= 0.0 || h <= 0.0 {
        return (String::new(), String::new());
    }
    let hi = if hi > 0.0 { hi } else { 1.0 };
    let n = values.len();
    let denom = (n - 1).max(1) as f64;
    let mut line = String::new();
    for (i, v) in values.iter().enumerate() {
        let x = i as f64 / denom * w;
        let y = h - (v / hi).clamp(0.0, 1.0) * h;
        if i == 0 {
            line.push_str(&format!("M{x:.2} {y:.2}"));
        } else {
            line.push_str(&format!(" L{x:.2} {y:.2}"));
        }
    }
    let area = format!("{line} L{w:.2} {h:.2} L0 {h:.2} Z");
    (line, area)
}

fn grad_id(color: &str) -> String {
    let mut h: u64 = 1469598103934665603;
    for b in color.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    format!("g{h:x}")
}

#[component]
pub fn Sparkline(
    values: Vec<f64>,
    #[props(default = 84.0)] width: f64,
    #[props(default = 22.0)] height: f64,
    #[props(default = "var(--accent)".to_string())] color: String,
    #[props(default = None)] max: Option<f64>,
) -> Element {
    let hi = max.unwrap_or_else(|| values.iter().copied().fold(0.0, f64::max).max(1.0));
    let (line, area) = spark_paths(&values, width, height, hi);
    let gid = grad_id(&color);
    rsx! {
        svg {
            class: "sparkline",
            width: "{width}",
            height: "{height}",
            view_box: "0 0 {width} {height}",
            preserve_aspect_ratio: "none",
            defs {
                linearGradient { id: "{gid}", x1: "0", y1: "0", x2: "0", y2: "1",
                    stop { offset: "0%", stop_color: "{color}", stop_opacity: "0.45" }
                    stop { offset: "100%", stop_color: "{color}", stop_opacity: "0.02" }
                }
            }
            path { d: "{area}", fill: "url(#{gid})", stroke: "none" }
            path { d: "{line}", fill: "none", stroke: "{color}", stroke_width: "1.5",
                stroke_linejoin: "round", stroke_linecap: "round" }
        }
    }
}

#[component]
pub fn Gauge(
    value: f64,
    #[props(default = 100.0)] max: f64,
    label: String,
    sub: String,
    #[props(default = "var(--accent)".to_string())] color: String,
    #[props(default = 104.0)] size: f64,
) -> Element {
    let r = size / 2.0 - 7.0;
    let circ = 2.0 * std::f64::consts::PI * r;
    let frac = if max > 0.0 {
        (value / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let offset = circ * (1.0 - frac);
    rsx! {
        div { class: "gauge", style: "width: {size}px; height: {size}px;",
            svg { width: "{size}", height: "{size}", view_box: "0 0 {size} {size}",
                circle {
                    cx: "{size / 2.0}", cy: "{size / 2.0}", r: "{r}",
                    fill: "none", stroke: "var(--border)", stroke_width: "6",
                }
                circle {
                    class: "ring",
                    cx: "{size / 2.0}", cy: "{size / 2.0}", r: "{r}",
                    fill: "none", stroke: "{color}", stroke_width: "6",
                    stroke_linecap: "round",
                    stroke_dasharray: "{circ}",
                    stroke_dashoffset: "{offset}",
                }
            }
            div { class: "gauge-val",
                div { class: "gauge-num", style: "color: {color};", "{label}" }
                div { class: "gauge-sub", "{sub}" }
            }
        }
    }
}

#[component]
pub fn HeatGrid(cores: Vec<CoreSample>, #[props(default = true)] show_freq: bool) -> Element {
    rsx! {
        div { class: "heat",
            for c in cores.iter() {
                div {
                    class: "cell",
                    style: heat_style(c.usage_pct),
                    title: format!("{:.0}%", c.usage_pct),
                    span { style: "font-weight:700;", "{c.usage_pct:.0}" }
                    if show_freq {
                        if let Some(f) = c.freq_mhz {
                            span { style: "color: var(--fg-dim);", "{f as f64 / 1000.0:.1}G" }
                        }
                    }
                }
            }
        }
    }
}

fn heat_style(usage: f64) -> String {
    let u = (usage / 100.0).clamp(0.0, 1.0);
    // Green → yellow → red as load rises; alpha scales with load.
    let color = if u < 0.5 {
        "var(--green)"
    } else if u < 0.8 {
        "var(--yellow)"
    } else {
        "var(--red)"
    };
    format!("background: rgba(0,0,0,0); box-shadow: inset 0 0 0 999px rgba(0,0,0,0); background-color: color-mix(in srgb, {color} {:.0}%, var(--bg-alt));", u * 100.0)
}

#[component]
pub fn StackBar(mem: MemoryBreakdown, sum_pss: u64) -> Element {
    let total = mem.total.max(1) as f64;
    let kernel_pct = mem.kernel as f64 / total * 100.0;
    let shared_pct = mem.shared as f64 / total * 100.0;
    // We draw apps = used_by_apps (capped so the bar never overflows), then shared, kernel,
    // cache and free. Overflow is clamped and any remainder is free.
    let apps = mem.used_by_apps().max(sum_pss.min(mem.used_by_apps())) as f64 / total * 100.0;
    let cache_pct = mem.cache_reclaimable as f64 / total * 100.0;
    let free_pct = (100.0 - apps - shared_pct - kernel_pct - cache_pct).max(0.0);
    rsx! {
        div { class: "stackbar",
            if apps > 0.3 {
                div { class: "seg used", style: "width: {apps:.3}%;",
                    if apps > 6.0 { "{human_kb(mem.used_by_apps())}" }
                }
            }
            if shared_pct > 0.3 {
                div { class: "seg shared", style: "width: {shared_pct:.3}%;",
                    if shared_pct > 6.0 { "{human_kb(mem.shared)}" }
                }
            }
            if kernel_pct > 0.3 {
                div { class: "seg kernel", style: "width: {kernel_pct:.3}%;",
                    if kernel_pct > 6.0 { "{human_kb(mem.kernel)}" }
                }
            }
            if cache_pct > 0.3 {
                div { class: "seg cache", style: "width: {cache_pct:.3}%;",
                    if cache_pct > 8.0 { "cache {human_kb(mem.cache_reclaimable)}" }
                }
            }
            div { class: "seg free", style: "width: {free_pct:.3}%;",
                if free_pct > 8.0 { "free {human_kb(mem.free)}" }
            }
        }
        div { class: "legend",
            span { span { class: "swatch", style: "background: var(--accent);" } "used by apps" }
            span { span { class: "swatch", style: "background: var(--magenta);" } "shared (tmpfs)" }
            span { span { class: "swatch", style: "background: var(--orange);" } "kernel" }
            span { span { class: "swatch", style: "background: var(--green);" } "cache (reclaimable)" }
            span { span { class: "swatch", style: "background: var(--border);" } "free" }
        }
    }
}

#[component]
pub fn LegendRow(label: String, value: String, color: String) -> Element {
    rsx! {
        div { class: "row", style: "font-size: 0.86em;",
            span { class: "swatch", style: "background: {color}; width:10px; height:10px; border-radius:3px; display:inline-block;" }
            span { class: "dim", "{label}" }
            span { class: "spacer" }
            span { class: "mono-num", "{value}" }
        }
    }
}
