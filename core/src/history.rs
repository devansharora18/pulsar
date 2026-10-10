//! Ring-buffer history for every graphed series (120 samples by default).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ring {
    cap: usize,
    data: VecDeque<f64>,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Ring { cap, data: VecDeque::with_capacity(cap + 1) }
    }

    pub fn push(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        if self.data.len() == self.cap {
            self.data.pop_front();
        }
        self.data.push_back(v);
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn last(&self) -> Option<f64> {
        self.data.back().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        self.data.iter().copied()
    }

    pub fn max(&self) -> f64 {
        self.data.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    }

    /// Values normalised to `0..=1` against `lo..=hi` (for sparkline paths).
    pub fn normalized(&self, lo: f64, hi: f64) -> Vec<f64> {
        if (hi - lo).abs() < f64::EPSILON {
            return self.data.iter().map(|_| 0.5).collect();
        }
        self.data.iter().map(|v| ((v - lo) / (hi - lo)).clamp(0.0, 1.0)).collect()
    }
}

/// Named ring buffers making up the rolling history the UI draws.
#[derive(Debug, Clone)]
pub struct History {
    pub cap: usize,
    pub cpu: Ring,
    pub mem_used: Ring,
    pub mem_cache: Ring,
    pub swap: Ring,
    pub power: Ring,
    pub cpu_temp: Ring,
    pub gpu_temp: Ring,
    pub net_rx: Ring,
    pub net_tx: Ring,
    pub disk_read: Ring,
    pub disk_write: Ring,
    pub group_power: std::collections::HashMap<String, Ring>,
}

impl History {
    pub fn new(cap: usize) -> Self {
        let r = || Ring::new(cap);
        History {
            cap,
            cpu: r(),
            mem_used: r(),
            mem_cache: r(),
            swap: r(),
            power: r(),
            cpu_temp: r(),
            gpu_temp: r(),
            net_rx: r(),
            net_tx: r(),
            disk_read: r(),
            disk_write: r(),
            group_power: std::collections::HashMap::new(),
        }
    }

    pub fn push_group_power(&mut self, key: &str, w: f64) {
        let cap = self.cap;
        self.group_power.entry(key.to_string()).or_insert_with(|| Ring::new(cap)).push(w);
    }

    /// Drop group series whose groups have not appeared for a while (kept small).
    pub fn retain_groups(&mut self, alive: &std::collections::HashSet<String>) {
        self.group_power.retain(|k, _| alive.contains(k));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_evicts_oldest_at_capacity() {
        let mut r = Ring::new(3);
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            r.push(v);
        }
        assert_eq!(r.len(), 3);
        assert_eq!(r.iter().collect::<Vec<_>>(), vec![3.0, 4.0, 5.0]);
        assert_eq!(r.last(), Some(5.0));
    }

    #[test]
    fn non_finite_values_are_dropped() {
        let mut r = Ring::new(3);
        r.push(f64::NAN);
        r.push(f64::INFINITY);
        r.push(2.0);
        assert_eq!(r.iter().collect::<Vec<_>>(), vec![2.0]);
    }

    #[test]
    fn normalization_maps_into_unit_range() {
        let mut r = Ring::new(3);
        r.push(0.0);
        r.push(50.0);
        r.push(100.0);
        let n = r.normalized(0.0, 100.0);
        assert_eq!(n, vec![0.0, 0.5, 1.0]);
        // Degenerate range stays finite.
        let mut z = Ring::new(1);
        z.push(7.0);
        assert_eq!(z.normalized(7.0, 7.0), vec![0.5]);
    }
}
