//! CPU times, per-core usage, frequencies and load average.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuTimes {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuTimes {
    pub fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    /// Idle includes iowait: a core waiting on IO is not doing work.
    pub fn idle_all(&self) -> u64 {
        self.idle + self.iowait
    }

    pub fn delta(&self, prev: &CpuTimes) -> (u64, u64) {
        let total = self.total().saturating_sub(prev.total());
        let idle = self.idle_all().saturating_sub(prev.idle_all());
        (total, idle)
    }
}

/// Parse `/proc/stat` CPU lines, returning `("cpu", overall)` followed by `("cpu0", ...)`.
pub fn parse_proc_stat(text: &str) -> Vec<(String, CpuTimes)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut it = line.split_whitespace();
        let Some(label) = it.next() else { continue };
        let nums: Vec<u64> = it.filter_map(|x| x.parse().ok()).collect();
        let get = |i: usize| nums.get(i).copied().unwrap_or(0);
        out.push((
            label.to_string(),
            CpuTimes {
                user: get(0),
                nice: get(1),
                system: get(2),
                idle: get(3),
                iowait: get(4),
                irq: get(5),
                softirq: get(6),
                steal: get(7),
            },
        ));
    }
    out
}

/// Busy fraction in `0..=1` from a `(total_delta, idle_delta)` pair.
pub fn usage_from_delta(total_delta: u64, idle_delta: u64) -> f64 {
    if total_delta == 0 {
        0.0
    } else {
        (1.0 - idle_delta as f64 / total_delta as f64).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CoreSample {
    pub usage_pct: f64,
    pub freq_mhz: Option<u64>,
    pub temp_c: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuSnapshot {
    pub cores: Vec<CoreSample>,
    pub overall_usage_pct: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub model: String,
}

/// Keeps previous `/proc/stat` counters so it can report deltas.
#[derive(Debug, Default)]
pub struct CpuCollector {
    prev: Vec<CpuTimes>,
    model: String,
}

impl CpuCollector {
    pub fn new() -> Self {
        CpuCollector {
            prev: Vec::new(),
            model: read_model_name(),
        }
    }

    pub fn sample(&mut self) -> CpuSnapshot {
        let text = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        let rows = parse_proc_stat(&text);
        let (l1, l5, l15) = read_loadavg();

        let mut cores = Vec::new();
        let mut overall = 0.0;
        for (i, (label, cur)) in rows.iter().enumerate() {
            let usage = if let Some(prev) = self.prev.get(i) {
                let (t, idle) = cur.delta(prev);
                usage_from_delta(t, idle)
            } else {
                0.0
            };
            if label == "cpu" {
                overall = usage;
                continue;
            }
            let idx: usize = label.strip_prefix("cpu").and_then(|s| s.parse().ok()).unwrap_or(0);
            cores.push(CoreSample {
                usage_pct: usage * 100.0,
                freq_mhz: read_freq_mhz(idx),
                temp_c: None,
            });
        }
        self.prev = rows.into_iter().map(|(_, t)| t).collect();

        CpuSnapshot {
            cores,
            overall_usage_pct: overall * 100.0,
            load1: l1,
            load5: l5,
            load15: l15,
            model: self.model.clone(),
        }
    }
}

pub fn read_loadavg() -> (f64, f64, f64) {
    let text = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let mut it = text.split_whitespace();
    let g = |it: &mut std::str::SplitWhitespace| it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    (g(&mut it), g(&mut it), g(&mut it))
}

fn read_freq_mhz(core: usize) -> Option<u64> {
    let base = format!("/sys/devices/system/cpu/cpu{core}/cpufreq");
    for f in ["scaling_cur_freq", "cpuinfo_cur_freq"] {
        if let Ok(s) = std::fs::read_to_string(format!("{base}/{f}")) {
            if let Ok(khz) = s.trim().parse::<u64>() {
                return Some(khz / 1000);
            }
        }
    }
    None
}

fn read_model_name() -> String {
    let text = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            if let Some((_, v)) = rest.split_once(':') {
                return v.trim().to_string();
            }
        }
    }
    "CPU".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "\
cpu  100 20 30 400 50 0 10 0 0 0
cpu0 50 10 15 200 25 0 5 0 0 0
cpu1 50 10 15 200 25 0 5 0 0 0
intr 12345
";

    #[test]
    fn parses_overall_and_per_core() {
        let rows = parse_proc_stat(STAT);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, "cpu");
        assert_eq!(rows[0].1.user, 100);
        assert_eq!(rows[0].1.iowait, 50);
        assert_eq!(rows[1].0, "cpu0");
        assert_eq!(rows[2].1.idle, 200);
    }

    #[test]
    fn usage_from_two_snapshots() {
        let a = CpuTimes { idle: 100, system: 100, ..Default::default() };
        let b = CpuTimes { idle: 150, system: 150, ..Default::default() };
        let (t, idle) = b.delta(&a);
        assert_eq!(t, 100);
        assert_eq!(idle, 50);
        assert!((usage_from_delta(t, idle) - 0.5).abs() < 1e-9);
        assert_eq!(usage_from_delta(0, 0), 0.0);
    }

    #[test]
    fn malformed_stat_is_empty_not_panic() {
        assert!(parse_proc_stat("").is_empty());
        assert!(parse_proc_stat("cpu\n").len() == 1);
    }
}
