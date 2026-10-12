//! The sampler: one tick every `refresh_ms`, producing immutable [`SystemSnapshot`]s.
//!
//! Everything expensive is cached across ticks. Static per-pid data (exe, cmdline, cgroup
//! app id) is read once per new pid/starttime, not once per tick.

use std::collections::{HashMap, HashSet};
use std::thread::JoinHandle;

use tokio::sync::watch;

use crate::config::Config;
use crate::cpu::CpuCollector;
use crate::desktop::DesktopIndex;
use crate::disk::DiskCollector;
use crate::filesystem::Filesystem;
use crate::group::{assign_groups, build_groups, ProcSample};
use crate::history::History;
use crate::meminfo::MemoryBreakdown;
use crate::net::NetCollector;
use crate::power::{attribute_watts, PowerSampler};
use crate::pressure::Pressure;
use crate::process::{self, IoCounters, Stat, Status};
use crate::pss::ProcMemory;
use crate::snapshot::{SeriesView, SystemSnapshot};

#[derive(Debug, Clone, Default)]
struct CachedProc {
    starttime: u64,
    exe: Option<String>,
    cmdline: Option<String>,
    app_id: Option<String>,
    uid: u32,
}

#[derive(Debug, Default)]
struct PidCache {
    map: HashMap<i32, CachedProc>,
}

impl PidCache {
    fn get_or_read(&mut self, pid: i32, starttime: u64) -> CachedProc {
        let needs = self
            .map
            .get(&pid)
            .map(|c| c.starttime != starttime)
            .unwrap_or(true);
        if needs {
            let uid = Status::read(pid).map(|s| s.uid).unwrap_or(u32::MAX);
            let c = CachedProc {
                starttime,
                exe: process::read_exe(pid),
                cmdline: process::read_cmdline(pid),
                app_id: crate::cgroup::app_id_for_pid(pid),
                uid,
            };
            self.map.insert(pid, c);
        }
        // Reader can only be missing if `pid` vanished between the insert and here; the
        // default (no exe/cmdline) degrades gracefully instead of panicking.
        self.map.get(&pid).cloned().unwrap_or_default()
    }

    fn retain(&mut self, alive: &HashSet<i32>) {
        self.map.retain(|pid, _| alive.contains(pid));
    }
}

pub struct Sampler {
    config: Config,
    desktop: DesktopIndex,
    cpu: CpuCollector,
    net: NetCollector,
    disk: DiskCollector,
    power: PowerSampler,
    cache: PidCache,
    history: History,
    /// pid → (starttime, cumulative cpu ticks) from the previous tick.
    prev_ticks: HashMap<i32, (u64, u64)>,
    /// Cached per-pid memory/io, refreshed on a rotating schedule.
    mem_cache: HashMap<i32, crate::pss::ProcMemory>,
    io_cache: HashMap<i32, IoCounters>,
    /// pid → (RSS kB, seq) at the last `smaps_rollup` read.
    smaps_epoch: HashMap<i32, (u64, u64)>,
    seq: u64,
    clock_ticks: u64,
    page_size: u64,
    last: std::time::Instant,
    rapl_readable: bool,
}

impl Sampler {
    pub fn new(config: Config) -> Self {
        let desktop = DesktopIndex::load_default();
        let clock_ticks = process::clock_ticks_per_sec();
        let page_size = process::page_size();
        let history = History::new(config.history_len);
        let rapl_readable = crate::rapl::readable();
        Sampler {
            config: config.clone(),
            desktop,
            cpu: CpuCollector::new(),
            net: NetCollector::default(),
            disk: DiskCollector::default(),
            power: PowerSampler::new(config.tdp_w),
            cache: PidCache::default(),
            history,
            prev_ticks: HashMap::new(),
            mem_cache: HashMap::new(),
            io_cache: HashMap::new(),
            smaps_epoch: HashMap::new(),
            seq: 0,
            clock_ticks,
            page_size,
            last: std::time::Instant::now(),
            rapl_readable,
        }
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Produce one snapshot. Never returns `Err`; unreadable data degrades to honest zeros.
    pub fn tick(&mut self) -> SystemSnapshot {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64().max(0.001);
        self.last = now;
        self.seq += 1;

        let mem = MemoryBreakdown::read().unwrap_or_default();
        let pressure = Pressure::read("memory").unwrap_or_default();
        let cpu = self.cpu.sample();
        let power = self.power.sample(cpu.overall_usage_pct / 100.0);

        let temps = crate::hwmon::read_temps();
        let filesystems: Vec<Filesystem> = crate::filesystem::read_filesystems();
        let net = self.net.sample(dt);
        let disks = self.disk.sample(dt);

        // `smaps_rollup` scales with the number of VMAs and is the sampler's hot spot.
        // Only re-read it when a process's RSS actually moved (or every `MAX_PSS_AGE`
        // ticks as a floor), so stable processes cost nothing while active ones stay live.
        const RSS_THRESHOLD_KB: u64 = 512;
        const MAX_PSS_AGE: u64 = 40;
        let pids = read_pids();

        let mut processes: Vec<ProcSample> = Vec::with_capacity(pids.len());
        let mut new_ticks: HashMap<i32, (u64, u64)> = HashMap::with_capacity(pids.len());
        let mut alive: HashSet<i32> = HashSet::with_capacity(pids.len());

        for pid in pids {
            let Ok(stat) = Stat::parse(&read_file(format!("/proc/{pid}/stat"))) else {
                continue;
            };
            alive.insert(pid);
            let cached = self.cache.get_or_read(pid, stat.starttime);
            // Kernel threads have no user memory; do not spend a rollup read on them.
            let kernel_thread = cached.exe.is_none() && cached.cmdline.is_none();

            let prev = self
                .prev_ticks
                .get(&pid)
                .filter(|(st, _)| *st == stat.starttime);
            let proc_delta = prev
                .map(|(_, t)| stat.total_ticks().saturating_sub(*t))
                .unwrap_or(0);
            new_ticks.insert(pid, (stat.starttime, stat.total_ticks()));

            let cpu_pct = proc_delta as f64 / (dt * self.clock_ticks as f64) * 100.0;
            let power_w = attribute_watts(power.cpu_dynamic_w, proc_delta, cpu.total_delta);

            let rss_fallback = (stat.rss_pages.max(0) as u64) * self.page_size / 1024;
            let (last_rss, last_seq) = self.smaps_epoch.get(&pid).copied().unwrap_or((u64::MAX, 0));
            let changed = rss_fallback.abs_diff(last_rss) > RSS_THRESHOLD_KB;
            let stale = self.seq.saturating_sub(last_seq) >= MAX_PSS_AGE;
            let detailed =
                !kernel_thread && (changed || stale || !self.mem_cache.contains_key(&pid));
            let mem_pm = if detailed {
                let pm = ProcMemory::read(pid, rss_fallback, 0);
                self.mem_cache.insert(pid, pm);
                self.smaps_epoch.insert(pid, (rss_fallback, self.seq));
                self.io_cache
                    .insert(pid, IoCounters::read(pid).unwrap_or_default());
                pm
            } else {
                let mut pm = *self.mem_cache.get(&pid).unwrap_or(&ProcMemory::default());
                pm.rss = rss_fallback;
                // Fallback (non-rollup) processes track RSS directly.
                if !pm.from_rollup {
                    pm.pss = rss_fallback;
                    pm.uss = 0;
                }
                pm
            };
            let io = *self.io_cache.get(&pid).unwrap_or(&IoCounters::default());

            processes.push(ProcSample {
                pid,
                ppid: stat.ppid,
                starttime: stat.starttime,
                comm: stat.comm.clone(),
                exe: cached.exe.clone(),
                cmdline: cached.cmdline.clone(),
                app_id: cached.app_id.clone(),
                uid: cached.uid,
                threads: stat.num_threads,
                mem: mem_pm,
                io,
                cpu_pct,
                power_w,
            });
        }

        let assigned = assign_groups(&processes, &self.desktop);
        let mut groups = build_groups(&assigned, &processes);
        groups.sort_by_key(|g| std::cmp::Reverse(g.pss));
        let sum_pss: u64 = processes.iter().map(|p| p.mem.pss).sum();

        self.cache.retain(&alive);
        self.mem_cache.retain(|pid, _| alive.contains(pid));
        self.io_cache.retain(|pid, _| alive.contains(pid));
        self.smaps_epoch.retain(|pid, _| alive.contains(pid));
        self.prev_ticks = new_ticks;

        // History.
        self.history.cpu.push(cpu.overall_usage_pct);
        self.history
            .mem_used
            .push(mem.used_real as f64 / 1024.0 / 1024.0);
        self.history
            .mem_cache
            .push(mem.cache_reclaimable as f64 / 1024.0 / 1024.0);
        self.history
            .swap
            .push(mem.swap_used as f64 / 1024.0 / 1024.0);
        self.history.power.push(power.total_w);
        self.history
            .cpu_temp
            .push(crate::hwmon::cpu_temp_c().unwrap_or(f64::NAN));
        self.history
            .gpu_temp
            .push(crate::hwmon::gpu_temp_c().unwrap_or(f64::NAN));
        let (rx, tx) = net
            .iter()
            .fold((0.0, 0.0), |(a, b), i| (a + i.rx_bps, b + i.tx_bps));
        self.history.net_rx.push(rx);
        self.history.net_tx.push(tx);
        let (dr, dw) = disks
            .iter()
            .fold((0.0, 0.0), |(a, b), d| (a + d.read_bps, b + d.write_bps));
        self.history.disk_read.push(dr);
        self.history.disk_write.push(dw);

        let group_keys: HashSet<String> = groups.iter().map(|g| g.key.clone()).collect();
        for g in &groups {
            self.history.push_group_power(&g.key, g.power_w);
        }
        self.history.retain_groups(&group_keys);
        for g in &mut groups {
            g.spark = self
                .history
                .group_power
                .get(&g.key)
                .map(|r| r.iter().collect())
                .unwrap_or_default();
        }
        let series = SeriesView {
            cpu: self.history.cpu.iter().collect(),
            mem_used: self.history.mem_used.iter().collect(),
            mem_cache: self.history.mem_cache.iter().collect(),
            swap: self.history.swap.iter().collect(),
            power: self.history.power.iter().collect(),
            net_rx: self.history.net_rx.iter().collect(),
            net_tx: self.history.net_tx.iter().collect(),
            disk_read: self.history.disk_read.iter().collect(),
            disk_write: self.history.disk_write.iter().collect(),
            cpu_temp: self.history.cpu_temp.iter().collect(),
            gpu_temp: self.history.gpu_temp.iter().collect(),
        };

        SystemSnapshot {
            seq: self.seq,
            dt,
            mem,
            pressure,
            cpu,
            power,
            net,
            disks,
            temps,
            filesystems,
            proc_count: processes.len(),
            processes,
            groups,
            sum_pss,
            rapl_readable: self.rapl_readable,
            series,
        }
    }

    /// Spawn the sampler on a dedicated thread running a tokio current-thread runtime, and
    /// publish snapshots over a `watch` channel.
    pub fn spawn(config: Config) -> (watch::Receiver<SystemSnapshot>, JoinHandle<()>) {
        let (tx, rx) = watch::channel(SystemSnapshot::default());
        let interval = config.refresh_ms;
        let handle = std::thread::Builder::new()
            .name("pulsar-sampler".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build();
                let Ok(rt) = rt else { return };
                rt.block_on(async move {
                    let mut sampler = Sampler::new(config);
                    // First tick immediately so the UI is never empty.
                    let _ = tx.send(sampler.tick());
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
                        if tx.send(sampler.tick()).is_err() {
                            break;
                        }
                    }
                });
            })
            .expect("spawn sampler thread");
        (rx, handle)
    }
}

fn read_file(path: String) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// PIDs with a numeric `/proc` entry.
pub fn read_pids() -> Vec<i32> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return out;
    };
    for ent in rd.flatten() {
        if let Some(name) = ent.file_name().to_str() {
            if let Ok(pid) = name.parse::<i32>() {
                out.push(pid);
            }
        }
    }
    out
}
