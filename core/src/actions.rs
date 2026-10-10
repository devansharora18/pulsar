//! Process and system controls. Privileged operations go through `pkexec`, never silently.

use std::time::Duration;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};

use crate::meminfo::MemoryBreakdown;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: String,
    pub target: String,
    pub ok: bool,
    pub message: String,
    pub privileged: bool,
}

impl ActionResult {
    fn ok(action: &str, target: &str, message: impl Into<String>, privileged: bool) -> Self {
        ActionResult {
            action: action.into(),
            target: target.into(),
            ok: true,
            message: message.into(),
            privileged,
        }
    }
    fn err(action: &str, target: &str, message: impl Into<String>, privileged: bool) -> Self {
        ActionResult {
            action: action.into(),
            target: target.into(),
            ok: false,
            message: message.into(),
            privileged,
        }
    }
}

fn to_signal(sig: Signal, pids: &[i32]) -> usize {
    let mut sent = 0;
    for pid in pids {
        if kill(Pid::from_raw(*pid), sig).is_ok() {
            sent += 1;
        }
    }
    sent
}

/// SIGTERM a set of pids.
pub fn terminate(pids: &[i32]) -> ActionResult {
    let n = to_signal(Signal::SIGTERM, pids);
    if n == 0 {
        ActionResult::err("terminate", "group", "no signals delivered", false)
    } else {
        ActionResult::ok(
            "terminate",
            "group",
            format!("SIGTERM sent to {n} process(es)"),
            false,
        )
    }
}

/// SIGKILL a set of pids.
pub fn force_kill(pids: &[i32]) -> ActionResult {
    let n = to_signal(Signal::SIGKILL, pids);
    ActionResult::ok(
        "kill",
        "group",
        format!("SIGKILL sent to {n} process(es)"),
        false,
    )
}

/// SIGTERM now, then SIGKILL any survivors after `grace`.
pub fn terminate_then_kill(pids: Vec<i32>, grace: Duration) -> ActionResult {
    let n = to_signal(Signal::SIGTERM, &pids);
    let owned = pids.clone();
    std::thread::spawn(move || {
        std::thread::sleep(grace);
        let survivors: Vec<i32> = owned
            .iter()
            .copied()
            .filter(|pid| kill(Pid::from_raw(*pid), None).is_ok())
            .collect();
        if !survivors.is_empty() {
            to_signal(Signal::SIGKILL, &survivors);
        }
    });
    ActionResult::ok(
        "kill",
        "group",
        format!(
            "SIGTERM sent to {n}; SIGKILL in {}s for survivors",
            grace.as_secs()
        ),
        false,
    )
}

pub fn suspend(pids: &[i32]) -> ActionResult {
    let n = to_signal(Signal::SIGSTOP, pids);
    ActionResult::ok("suspend", "group", format!("SIGSTOP sent to {n}"), false)
}

pub fn resume(pids: &[i32]) -> ActionResult {
    let n = to_signal(Signal::SIGCONT, pids);
    ActionResult::ok("resume", "group", format!("SIGCONT sent to {n}"), false)
}

/// Renice a single process. `nice` is clamped to `-20..=19`; lowering below the current nice
/// requires privileges the caller may not have, which surfaces as an error.
pub fn renice(pid: i32, nice: i32) -> ActionResult {
    let nice = nice.clamp(-20, 19);
    // SAFETY: setpriority is a plain syscall wrapper.
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) };
    if rc == 0 {
        ActionResult::ok(
            "renice",
            &pid.to_string(),
            format!("nice set to {nice}"),
            false,
        )
    } else {
        let e = std::io::Error::last_os_error();
        ActionResult::err("renice", &pid.to_string(), format!("failed: {e}"), false)
    }
}

/// Whether swapping back in is safe: available RAM must comfortably exceed swap in use.
pub fn swap_flush_blocker(mem: &MemoryBreakdown) -> Option<String> {
    if mem.swap_used == 0 {
        return Some("nothing is swapped out".to_string());
    }
    // Require at least 1.5x the swap in use as headroom.
    let needed = mem.swap_used + mem.swap_used / 2;
    if mem.available < needed {
        return Some(format!(
            "needs ~{} free to swap back in, only {} available",
            human_kb(needed),
            human_kb(mem.available)
        ));
    }
    None
}

fn human_kb(kb: u64) -> String {
    let gb = kb as f64 / 1024.0 / 1024.0;
    format!("{gb:.1} GB")
}

/// Run a shell command through `pkexec`. Never silent: the caller logs the [`ActionResult`].
fn pkexec(action: &str, target: &str, sh: &str) -> ActionResult {
    match std::process::Command::new("pkexec")
        .args(["sh", "-c", sh])
        .status()
    {
        Ok(s) if s.success() => ActionResult::ok(action, target, "completed", true),
        Ok(s) => ActionResult::err(action, target, format!("pkexec exited with {s}"), true),
        Err(e) => ActionResult::err(action, target, format!("pkexec failed: {e}"), true),
    }
}

/// `echo 1 > /proc/sys/vm/drop_caches` (reclaimable cache only).
pub fn drop_caches() -> ActionResult {
    pkexec(
        "drop-caches",
        "system",
        "sync; echo 1 > /proc/sys/vm/drop_caches",
    )
}

/// `echo 3 > /proc/sys/vm/drop_caches` (cache + dentries + inodes).
pub fn drop_caches_all() -> ActionResult {
    pkexec(
        "drop-caches-all",
        "system",
        "sync; echo 3 > /proc/sys/vm/drop_caches",
    )
}

/// `echo 1 > /proc/sys/vm/compact_memory`.
pub fn compact_memory() -> ActionResult {
    pkexec(
        "compact-memory",
        "system",
        "echo 1 > /proc/sys/vm/compact_memory",
    )
}

/// `swapoff -a && swapon -a` — reads every swapped page back into RAM.
pub fn flush_swap() -> ActionResult {
    pkexec("flush-swap", "system", "swapoff -a && swapon -a")
}

/// Apply the shipped udev rule so RAPL becomes readable without root for future boots.
pub fn enable_power_metrics() -> ActionResult {
    let rule = "cp /usr/share/pulsar/99-pulsar-rapl.rules /etc/udev/rules.d/ 2>/dev/null || true; \
                printf '%s\\n' 'SUBSYSTEM==\"powercap\", ACTION==\"add|change\", RUN+=\"/bin/chgrp wheel /sys/class/powercap/intel-rapl:*/energy_uj\", RUN+=\"/bin/chmod g+r /sys/class/powercap/intel-rapl:*/energy_uj\"' > /etc/udev/rules.d/99-pulsar-rapl.rules; \
                udevadm control --reload-rules && udevadm trigger --subsystem-match=powercap";
    pkexec("enable-power-metrics", "system", rule)
}

/// Open the directory containing `path` in the desktop file manager.
pub fn open_exe_location(path: &str) -> ActionResult {
    let dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string());
    match std::process::Command::new("xdg-open").arg(&dir).spawn() {
        Ok(_) => ActionResult::ok("open-location", &dir, "opened", false),
        Err(e) => ActionResult::err("open-location", &dir, format!("{e}"), false),
    }
}

/// Check whether RAPL is currently readable (used to decide the "enable" affordance).
pub fn rapl_readable() -> bool {
    crate::rapl::readable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_flush_blocker_reasons() {
        let mut m = MemoryBreakdown {
            swap_used: 0,
            available: 1_000_000,
            ..Default::default()
        };
        assert!(swap_flush_blocker(&m).is_some());
        // Plenty of RAM.
        m.swap_used = 1_000_000;
        m.available = 5_000_000;
        assert!(swap_flush_blocker(&m).is_none());
        // Not enough RAM.
        m.available = 1_100_000;
        assert!(swap_flush_blocker(&m).is_some());
    }

    #[test]
    fn renice_clamps_and_reports() {
        // Renicing our own process to a valid value should succeed.
        let me = std::process::id() as i32;
        let r = renice(me, 5);
        assert_eq!(r.action, "renice");
        // Out-of-range is clamped, not an error path.
        let r = renice(me, 999);
        assert!(r.message.contains("19"));
    }

    #[test]
    fn terminate_ignores_dead_pids() {
        // An impossible pid just fails to deliver a signal; no panic.
        let r = terminate(&[i32::MAX - 1]);
        assert!(!r.ok);
    }
}
