//! RAPL energy counters (`/sys/class/powercap/intel-rapl*/energy_uj`).
//!
//! Since Linux 5.10 these files are root-only. Pulsar reads them when it can and falls back
//! to a model otherwise; the wrapping counter is handled here.

use std::path::{Path, PathBuf};

/// Wraparound-safe difference between two `energy_uj` readings.
///
/// The counter rolls over at `max_range`; a naive subtraction would produce a huge spike.
pub fn energy_delta(prev: u64, cur: u64, max_range: u64) -> u64 {
    if cur >= prev {
        cur - prev
    } else if max_range > prev {
        cur + (max_range - prev)
    } else {
        0
    }
}

/// Microjoules over microseconds → watts.
pub fn uj_per_us_to_watts(energy_uj: u64, elapsed_us: u64) -> f64 {
    if elapsed_us == 0 {
        0.0
    } else {
        energy_uj as f64 / elapsed_us as f64
    }
}

#[derive(Debug, Clone)]
pub struct RaplZone {
    pub name: String,
    pub path: PathBuf,
    /// A top-level package zone (`intel-rapl:0`), as opposed to a subzone like `core`.
    pub is_package: bool,
}

impl RaplZone {
    pub fn max_range(&self) -> u64 {
        std::fs::read_to_string(self.path.join("max_energy_range_uj"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    pub fn read_energy(&self) -> Option<u64> {
        let s = std::fs::read_to_string(self.path.join("energy_uj")).ok()?;
        s.trim().parse().ok()
    }
}

/// Enumerate RAPL zones. Empty when the kernel exposes none (`amd_energy` absent, old CPU).
pub fn list_zones() -> Vec<RaplZone> {
    let mut zones = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/powercap") else {
        return zones;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let dirname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !dirname.contains("rapl") {
            continue;
        }
        if !path.join("energy_uj").exists() {
            continue;
        }
        let name = std::fs::read_to_string(path.join("name"))
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| dirname.to_string());
        // `intel-rapl:0` is a package zone; `intel-rapl:0:0` is a subzone.
        let is_package = dirname.matches(':').count() == 1;
        zones.push(RaplZone {
            name,
            path,
            is_package,
        });
    }
    zones.sort_by(|a, b| a.path.cmp(&b.path));
    zones
}

/// True if the energy file is readable by the current user.
pub fn readable() -> bool {
    list_zones()
        .iter()
        .any(|z| z.is_package && z.read_energy().is_some())
}

/// Read the total package power (sum over top-level package zones) as watts.
pub struct RaplReader {
    zones: Vec<RaplZone>,
    last: Vec<(u64, std::time::Instant)>,
    max_ranges: Vec<u64>,
}

impl std::fmt::Debug for RaplReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaplReader")
            .field("zones", &self.zones.len())
            .finish()
    }
}

impl RaplReader {
    pub fn new() -> Self {
        let zones: Vec<RaplZone> = list_zones().into_iter().filter(|z| z.is_package).collect();
        let max_ranges = zones.iter().map(|z| z.max_range()).collect();
        let last = vec![(0, std::time::Instant::now()); zones.len()];
        RaplReader {
            zones,
            last,
            max_ranges,
        }
    }

    pub fn available(&self) -> bool {
        !self.zones.is_empty()
    }

    /// Read package watts since the previous call. Returns `None` on the first call (no
    /// baseline yet) or when nothing is readable.
    pub fn sample(&mut self) -> Option<f64> {
        if self.zones.is_empty() {
            return None;
        }
        let now = std::time::Instant::now();
        let mut total = 0.0;
        let mut any = false;
        for (i, z) in self.zones.iter().enumerate() {
            let Some(cur) = z.read_energy() else { continue };
            let (prev, prev_t) = self.last[i];
            let max = self.max_ranges[i];
            if prev != 0 {
                let elapsed_us = now.duration_since(prev_t).as_micros() as u64;
                let delta = energy_delta(prev, cur, max);
                if elapsed_us > 0 {
                    total += uj_per_us_to_watts(delta, elapsed_us);
                }
                any = true;
            }
            self.last[i] = (cur, now);
        }
        if any {
            Some(total)
        } else {
            None
        }
    }
}

impl Default for RaplReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Read an amdgpu GPU power average (`power1_average`, microwatts) if present.
pub fn read_amdgpu_power_w() -> Option<f64> {
    let rd = std::fs::read_dir("/sys/class/drm").ok()?;
    for ent in rd.flatten() {
        let p = ent.path();
        let fname = p.file_name()?.to_str()?.to_string();
        if !fname.starts_with("card") || fname.contains('-') {
            continue;
        }
        let hwmon = p.join("device/hwmon");
        let Ok(hd) = std::fs::read_dir(&hwmon) else {
            continue;
        };
        for h in hd.flatten() {
            if let Ok(v) = std::fs::read_to_string(h.path().join("power1_average")) {
                if let Ok(uw) = v.trim().parse::<u64>() {
                    return Some(uw as f64 / 1_000_000.0);
                }
            }
        }
    }
    None
}

/// Read battery power draw. Prefers `power_now`; falls back to `current_now * voltage_now`.
pub fn read_battery_power_w() -> Option<f64> {
    let rd = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for ent in rd.flatten() {
        let p = ent.path();
        let ty = std::fs::read_to_string(p.join("type")).unwrap_or_default();
        if !ty.trim().eq_ignore_ascii_case("battery") {
            continue;
        }
        if let Ok(v) = std::fs::read_to_string(p.join("power_now")) {
            if let Ok(uw) = v.trim().parse::<i64>() {
                return Some(uw.unsigned_abs() as f64 / 1_000_000.0);
            }
        }
        let curr = std::fs::read_to_string(p.join("current_now"))
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok());
        let volt = std::fs::read_to_string(p.join("voltage_now"))
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok());
        if let (Some(c), Some(vv)) = (curr, volt) {
            // µA * µV = µW
            return Some(
                (c.unsigned_abs() as f64 * vv.unsigned_abs() as f64) / 1_000_000_000_000.0,
            );
        }
    }
    None
}

/// Read a TDP heuristic from RAPL constraint files, else `None`.
pub fn read_tdp_w() -> Option<f64> {
    let rd = std::fs::read_dir("/sys/class/powercap").ok()?;
    for ent in rd.flatten() {
        let p = ent.path();
        if !p.file_name()?.to_str()?.contains("rapl") {
            continue;
        }
        for c in ["constraint_0_power_limit_uw", "constraint_1_power_limit_uw"] {
            if let Ok(v) = std::fs::read_to_string(p.join(c)) {
                if let Ok(uw) = v.trim().parse::<u64>() {
                    if uw > 1_000_000 {
                        return Some(uw as f64 / 1_000_000.0);
                    }
                }
            }
        }
    }
    None
}

pub fn path_is_readable(p: &Path) -> bool {
    std::fs::File::open(p).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraparound_is_handled() {
        // prev just under the wrap point, cur just after.
        assert_eq!(energy_delta(100, 150, 1000), 50);
        assert_eq!(energy_delta(990, 10, 1000), 20);
        assert_eq!(energy_delta(500, 500, 1000), 0);
        // Degenerate range must not underflow.
        assert_eq!(energy_delta(990, 10, 0), 0);
    }

    #[test]
    fn watts_from_energy_and_time() {
        // 1 J in 1 s == 1 W.
        assert_eq!(uj_per_us_to_watts(1_000_000, 1_000_000), 1.0);
        assert_eq!(uj_per_us_to_watts(500_000, 1_000_000), 0.5);
        assert_eq!(uj_per_us_to_watts(123, 0), 0.0);
    }

    #[test]
    fn reader_without_zones_is_unavailable() {
        let mut r = RaplReader {
            zones: vec![],
            last: vec![],
            max_ranges: vec![],
        };
        assert!(!r.available());
        assert_eq!(r.sample(), None);
    }
}
