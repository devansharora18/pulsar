//! Network interface byte counters from `/sys/class/net`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub up: bool,
    pub is_virtual: bool,
}

/// Per-second rate from two cumulative counters over `dt` seconds.
pub fn rate(prev: u64, cur: u64, dt: f64) -> f64 {
    if dt <= 0.0 || cur < prev {
        0.0
    } else {
        (cur - prev) as f64 / dt
    }
}

#[derive(Debug, Default)]
pub struct NetCollector {
    prev: Vec<(String, u64, u64)>,
}

impl NetCollector {
    pub fn sample(&mut self, dt: f64) -> Vec<NetInterface> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir("/sys/class/net") else { return out };
        let mut ifaces: Vec<(String, u64, u64, bool, bool)> = Vec::new();
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            let base = ent.path();
            let rx = read_u64(&base.join("statistics/rx_bytes"));
            let tx = read_u64(&base.join("statistics/tx_bytes"));
            let operstate = std::fs::read_to_string(base.join("operstate")).unwrap_or_default();
            let up = operstate.trim() == "up";
            // A virtual interface has no `device` symlink.
            let is_virtual = !base.join("device").exists();
            ifaces.push((name, rx, tx, up, is_virtual));
        }
        ifaces.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, rx, tx, up, is_virtual) in ifaces {
            let (rx_bps, tx_bps) = if let Some((_, prx, ptx)) = self.prev.iter().find(|p| p.0 == name) {
                (rate(*prx, rx, dt), rate(*ptx, tx, dt))
            } else {
                (0.0, 0.0)
            };
            out.push(NetInterface { name, rx_bytes: rx, tx_bytes: tx, rx_bps, tx_bps, up, is_virtual });
        }
        self.prev = out.iter().map(|i| (i.name.clone(), i.rx_bytes, i.tx_bytes)).collect();
        out
    }
}

fn read_u64(p: &std::path::Path) -> u64 {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_math() {
        assert_eq!(rate(1000, 3000, 2.0), 1000.0);
        assert_eq!(rate(1000, 1000, 1.0), 0.0);
        // Counter reset must not go negative.
        assert_eq!(rate(5000, 100, 1.0), 0.0);
        assert_eq!(rate(0, 100, 0.0), 0.0);
    }
}
