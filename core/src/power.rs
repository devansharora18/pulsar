//! Power estimation and honest attribution.
//!
//! There is no per-process power meter on Linux, so Pulsar estimates. CPU watts are the
//! package's *dynamic* draw (`package − idle_baseline`) apportioned by each process's share
//! of total CPU ticks. GPU watts are shown at system level unless a driver exposes
//! per-process utilisation. Everything is labelled `est.` or `model` in the UI.

use serde::{Deserialize, Serialize};

use crate::rapl::RaplReader;

/// Which method produced the CPU figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerKind {
    /// Measured from RAPL energy counters.
    Rapl,
    /// `utilisation × TDP` model (RAPL unreadable).
    Model,
    /// Neither available.
    Unavailable,
}

/// A model fallback TDP used when the kernel exposes no constraint files and config is unset.
pub const DEFAULT_TDP_W: f64 = 28.0;

/// Tracks the lowest sustained package power, drifting up slowly so the "idle baseline"
/// follows the machine (fan curve, ambient, background load) instead of being stuck forever.
#[derive(Debug, Clone)]
pub struct Baseline {
    value: f64,
    initialized: bool,
    /// Per-observation upward drift; deliberately tiny (0.2%) for a 1 Hz sampler.
    alpha: f64,
}

impl Baseline {
    pub fn new() -> Self {
        Baseline { value: 0.0, initialized: false, alpha: 0.002 }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn observe(&mut self, watts: f64) {
        if !self.initialized {
            self.value = watts;
            self.initialized = true;
            return;
        }
        if watts < self.value {
            self.value = watts;
        } else {
            self.value += (watts - self.value) * self.alpha;
        }
    }
}

impl Default for Baseline {
    fn default() -> Self {
        Self::new()
    }
}

/// Apportion `dynamic_w` to a process by its share of the interval's CPU ticks.
pub fn attribute_watts(dynamic_w: f64, part_ticks: u64, whole_ticks: u64) -> f64 {
    if whole_ticks == 0 || dynamic_w <= 0.0 {
        0.0
    } else {
        dynamic_w * part_ticks as f64 / whole_ticks as f64
    }
}

/// Dynamic (non-idle) package watts.
pub fn cpu_dynamic_watts(pkg_w: f64, baseline_w: f64) -> f64 {
    (pkg_w - baseline_w).max(0.0)
}

/// The fallback model: utilisation × TDP, clamped to sane bounds.
pub fn model_watts(utilization: f64, tdp_w: f64) -> f64 {
    utilization.clamp(0.0, 1.0) * tdp_w
}

/// Snapshot of system-level power.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemPower {
    pub total_w: f64,
    pub pkg_w: Option<f64>,
    pub cpu_dynamic_w: f64,
    pub baseline_w: f64,
    pub gpu_w: Option<f64>,
    pub battery_w: Option<f64>,
    pub kind: PowerKind,
}

impl Default for SystemPower {
    fn default() -> Self {
        SystemPower {
            total_w: 0.0,
            pkg_w: None,
            cpu_dynamic_w: 0.0,
            baseline_w: 0.0,
            gpu_w: None,
            battery_w: None,
            kind: PowerKind::Unavailable,
        }
    }
}

/// Reads every available power source once per tick.
pub struct PowerSampler {
    rapl: RaplReader,
    baseline: Baseline,
    tdp_w: f64,
}

impl PowerSampler {
    pub fn new(tdp_w: Option<f64>) -> Self {
        let tdp = tdp_w
            .or_else(crate::rapl::read_tdp_w)
            .unwrap_or(DEFAULT_TDP_W);
        PowerSampler { rapl: RaplReader::new(), baseline: Baseline::new(), tdp_w: tdp }
    }

    pub fn tdp_w(&self) -> f64 {
        self.tdp_w
    }

    /// Sample, given the interval's overall CPU utilisation (0..=1) for the model fallback.
    pub fn sample(&mut self, cpu_util: f64) -> SystemPower {
        let mut kind = PowerKind::Unavailable;
        let mut pkg_w = None;

        if self.rapl.available() {
            if let Some(pkg) = self.rapl.sample() {
                kind = PowerKind::Rapl;
                pkg_w = Some(pkg);
            }
        }
        if pkg_w.is_none() {
            // Model fallback.
            kind = PowerKind::Model;
            pkg_w = Some(model_watts(cpu_util, self.tdp_w));
        }

        let pkg = pkg_w.unwrap_or(0.0);
        self.baseline.observe(pkg);
        let dynamic = if kind == PowerKind::Rapl {
            cpu_dynamic_watts(pkg, self.baseline.value())
        } else {
            // For the model there is no measured idle; attribute the whole model draw.
            pkg
        };

        let gpu_w = crate::rapl::read_amdgpu_power_w();
        let battery_w = crate::rapl::read_battery_power_w();

        let total_w = match battery_w {
            Some(b) if b > 0.5 => b,
            _ => pkg + gpu_w.unwrap_or(0.0),
        };

        SystemPower {
            total_w,
            pkg_w: Some(pkg),
            cpu_dynamic_w: dynamic,
            baseline_w: self.baseline.value(),
            gpu_w,
            battery_w,
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribution_is_proportional() {
        // 40 W dynamic, a process with 25% of ticks gets 10 W.
        assert_eq!(attribute_watts(40.0, 25, 100), 10.0);
        assert_eq!(attribute_watts(40.0, 0, 100), 0.0);
        assert_eq!(attribute_watts(40.0, 50, 0), 0.0);
        assert_eq!(attribute_watts(-5.0, 50, 100), 0.0);
        // Shares sum to the dynamic draw.
        let a = attribute_watts(40.0, 30, 100);
        let b = attribute_watts(40.0, 70, 100);
        assert!((a + b - 40.0).abs() < 1e-9);
    }

    #[test]
    fn dynamic_never_negative() {
        assert_eq!(cpu_dynamic_watts(5.0, 8.0), 0.0);
        assert_eq!(cpu_dynamic_watts(20.0, 8.0), 12.0);
    }

    #[test]
    fn baseline_follows_idle_down_and_drifts_up() {
        let mut b = Baseline::new();
        b.observe(30.0);
        assert_eq!(b.value(), 30.0);
        b.observe(60.0);
        assert!(b.value() > 30.0 && b.value() < 31.0, "got {}", b.value());
        b.observe(12.0);
        assert_eq!(b.value(), 12.0);
    }

    #[test]
    fn model_clamps_utilisation() {
        assert_eq!(model_watts(0.5, 40.0), 20.0);
        assert_eq!(model_watts(2.0, 40.0), 40.0);
        assert_eq!(model_watts(-1.0, 40.0), 0.0);
    }

    #[test]
    fn sampler_never_panics_without_rapl() {
        let mut s = PowerSampler::new(Some(15.0));
        let p = s.sample(0.4);
        assert!(p.pkg_w.is_some());
        assert!(p.total_w >= 0.0);
    }
}
