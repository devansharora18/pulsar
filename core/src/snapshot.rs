//! The immutable per-tick snapshot handed from the sampler to the UI.

use serde::{Deserialize, Serialize};

use crate::cpu::CpuSnapshot;
use crate::disk::DiskStat;
use crate::filesystem::Filesystem;
use crate::group::{Group, ProcSample};
use crate::hwmon::TempSensor;
use crate::meminfo::MemoryBreakdown;
use crate::net::NetInterface;
use crate::power::SystemPower;
use crate::pressure::Pressure;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub seq: u64,
    /// Seconds since the previous snapshot.
    pub dt: f64,
    pub mem: MemoryBreakdown,
    pub pressure: Pressure,
    pub cpu: CpuSnapshot,
    pub power: SystemPower,
    pub net: Vec<NetInterface>,
    pub disks: Vec<DiskStat>,
    pub temps: Vec<TempSensor>,
    pub filesystems: Vec<Filesystem>,
    pub processes: Vec<ProcSample>,
    pub groups: Vec<Group>,
    /// Sum of PSS over all processes — should approach `mem.used_by_apps()`.
    pub sum_pss: u64,
    pub proc_count: usize,
    /// The RAPL capability probe result (for the "Enable power metrics" affordance).
    pub rapl_readable: bool,
}

impl Default for SystemSnapshot {
    fn default() -> Self {
        SystemSnapshot {
            seq: 0,
            dt: 1.0,
            mem: MemoryBreakdown::default(),
            pressure: Pressure::default(),
            cpu: CpuSnapshot::default(),
            power: SystemPower::default(),
            net: Vec::new(),
            disks: Vec::new(),
            temps: Vec::new(),
            filesystems: Vec::new(),
            processes: Vec::new(),
            groups: Vec::new(),
            sum_pss: 0,
            proc_count: 0,
            rapl_readable: false,
        }
    }
}

impl SystemSnapshot {
    /// Memory used that no bucket accounts for — shown explicitly so nothing contradicts.
    pub fn unattributed_mem(&self) -> u64 {
        self.mem.unattributed(self.sum_pss)
    }

    pub fn process_by_pid(&self, pid: i32) -> Option<&ProcSample> {
        self.processes.iter().find(|p| p.pid == pid)
    }
}

/// Every sample value the UI keeps in memory, serialised for reuse by the `--screenshot`
/// mode's offline renderer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeriesView {
    pub cpu: Vec<f64>,
    pub mem_used: Vec<f64>,
    pub mem_cache: Vec<f64>,
    pub swap: Vec<f64>,
    pub power: Vec<f64>,
    pub net_rx: Vec<f64>,
    pub net_tx: Vec<f64>,
    pub disk_read: Vec<f64>,
    pub disk_write: Vec<f64>,
    pub cpu_temp: Vec<f64>,
    pub gpu_temp: Vec<f64>,
}
