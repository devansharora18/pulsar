//! `/proc/meminfo` parsing and the *honest* memory breakdown Pulsar shows.
//!
//! The whole point is that every number the UI displays reconciles. RSS-style sums are
//! avoided in favour of PSS (see [`crate::pss`]) and the system figure is decomposed into
//! mutually explained buckets.

use serde::{Deserialize, Serialize};

/// Raw `/proc/meminfo`, all values in kibibytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemInfo {
    pub mem_total: u64,
    pub mem_free: u64,
    pub mem_available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_cached: u64,
    pub shmem: u64,
    pub sreclaimable: u64,
    pub sunreclaim: u64,
    pub slab: u64,
    pub kreclaimable: u64,
    pub kernel_stack: u64,
    pub page_tables: u64,
    pub vmalloc_used: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub zswap: u64,
    pub zswapped: u64,
    pub dirty: u64,
    pub writeback: u64,
}

impl MemInfo {
    /// Parse the text of a `/proc/meminfo` file. Unknown keys are ignored and missing keys
    /// default to zero so a truncated or odd kernel never makes the UI crash.
    pub fn parse(text: &str) -> Self {
        let mut m = MemInfo::default();
        for line in text.lines() {
            let Some((key, rest)) = line.split_once(':') else {
                continue;
            };
            let Some(first) = rest.split_whitespace().next() else {
                continue;
            };
            let Ok(val) = first.parse::<u64>() else {
                continue;
            };
            match key {
                "MemTotal" => m.mem_total = val,
                "MemFree" => m.mem_free = val,
                "MemAvailable" => m.mem_available = val,
                "Buffers" => m.buffers = val,
                "Cached" => m.cached = val,
                "SwapCached" => m.swap_cached = val,
                "Shmem" => m.shmem = val,
                "SReclaimable" => m.sreclaimable = val,
                "SUnreclaim" => m.sunreclaim = val,
                "Slab" => m.slab = val,
                "KReclaimable" => m.kreclaimable = val,
                "KernelStack" => m.kernel_stack = val,
                "PageTables" => m.page_tables = val,
                "VmallocUsed" => m.vmalloc_used = val,
                "SwapTotal" => m.swap_total = val,
                "SwapFree" => m.swap_free = val,
                "Zswap" => m.zswap = val,
                "Zswapped" => m.zswapped = val,
                "Dirty" => m.dirty = val,
                "Writeback" => m.writeback = val,
                _ => {}
            }
        }
        m
    }

    pub fn read() -> crate::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string("/proc/meminfo")?))
    }
}

/// The decomposed memory picture. All values in kibibytes.
///
/// The buckets are designed so the stacked bar adds up to `total`:
/// `used_real + cache_reclaimable + free` slightly overshoots because `used_real` already
/// contains the reclaimable cache; the UI renders `used_by_apps` (system used minus kernel
/// and unattributed, clamped) instead, and lists `kernel` and `unattributed` explicitly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBreakdown {
    /// `MemTotal`.
    pub total: u64,
    /// `MemAvailable` — what apps can take without swapping.
    pub available: u64,
    /// `MemTotal - MemAvailable` — the "really used" figure GNOME gets wrong.
    pub used_real: u64,
    /// `Cached + Buffers + SReclaimable - Shmem` — reclaimable under pressure.
    pub cache_reclaimable: u64,
    /// `Shmem` — tmpfs / shared memory that is *not* reclaimable like cache.
    pub shared: u64,
    /// `SUnreclaim + KernelStack + PageTables + VmallocUsed`.
    pub kernel: u64,
    /// `MemFree`.
    pub free: u64,
    /// `SwapTotal - SwapFree`.
    pub swap_used: u64,
    pub swap_total: u64,
}

impl MemoryBreakdown {
    pub fn from_meminfo(m: &MemInfo) -> Self {
        let cache_reclaimable = m
            .cached
            .saturating_add(m.buffers)
            .saturating_add(m.sreclaimable)
            .saturating_sub(m.shmem);
        let kernel = m
            .sunreclaim
            .saturating_add(m.kernel_stack)
            .saturating_add(m.page_tables)
            .saturating_add(m.vmalloc_used);
        MemoryBreakdown {
            total: m.mem_total,
            available: m.mem_available.min(m.mem_total),
            used_real: m.mem_total.saturating_sub(m.mem_available),
            cache_reclaimable,
            shared: m.shmem,
            kernel,
            free: m.mem_free,
            swap_used: m.swap_total.saturating_sub(m.swap_free),
            swap_total: m.swap_total,
        }
    }

    pub fn read() -> crate::Result<Self> {
        Ok(Self::from_meminfo(&MemInfo::read()?))
    }

    /// Fraction of total that is really used, in `0.0..=1.0`.
    pub fn used_fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.used_real as f64 / self.total as f64
        }
    }

    /// The portion of "really used" memory left once the kernel's own bookkeeping is
    /// removed. This is what app PSS sums should approach.
    pub fn used_by_apps(&self) -> u64 {
        self.used_real.saturating_sub(self.kernel)
    }

    /// Memory used that no bucket explains: `used_real - kernel - sum(app PSS)`.
    /// Rendered as the explicit "kernel + unattributed" row so the story always adds up.
    pub fn unattributed(&self, sum_app_pss: u64) -> u64 {
        self.used_real
            .saturating_sub(self.kernel)
            .saturating_sub(sum_app_pss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
MemTotal:       34497536 kB
MemFree:         4044188 kB
MemAvailable:   23697204 kB
Buffers:          100000 kB
Cached:         19902000 kB
SwapCached:          156 kB
Shmem:           1554768 kB
SReclaimable:    2000000 kB
SUnreclaim:       558908 kB
Slab:            2558908 kB
KReclaimable:    2000000 kB
KernelStack:       28508 kB
PageTables:        83120 kB
VmallocUsed:      126080 kB
SwapTotal:      75497464 kB
SwapFree:       74354184 kB
Zswap:                 0 kB
Zswapped:              0 kB
Dirty:              6824 kB
";

    #[test]
    fn parses_all_known_fields() {
        let m = MemInfo::parse(FIXTURE);
        assert_eq!(m.mem_total, 34497536);
        assert_eq!(m.mem_available, 23697204);
        assert_eq!(m.cached, 19902000);
        assert_eq!(m.sreclaimable, 2000000);
        assert_eq!(m.sunreclaim, 558908);
        assert_eq!(m.shmem, 1554768);
        assert_eq!(m.swap_total, 75497464);
        assert_eq!(m.swap_free, 74354184);
        assert_eq!(m.zswap, 0);
    }

    #[test]
    fn breakdown_is_internally_consistent() {
        let m = MemInfo::parse(FIXTURE);
        let b = MemoryBreakdown::from_meminfo(&m);
        assert_eq!(b.total, 34497536);
        assert_eq!(b.used_real, 10800332); // 10.30 GiB
        assert_eq!(b.cache_reclaimable, 20447232); // 19.5 GiB
        assert_eq!(b.shared, 1554768);
        assert_eq!(b.kernel, 558908 + 28508 + 83120 + 126080);
        assert_eq!(b.free, 4044188);
        assert_eq!(b.swap_used, 1143280);
        // The core invariant GNOME violates: used + available == total.
        assert_eq!(b.used_real + b.available, b.total);
    }

    #[test]
    fn the_103_gb_case_reads_as_103_gb() {
        // Reproduces the reported contradiction: header said 10.3 GB used / 31.4%, list
        // summed ~3.4 GB. The header figure is correct; the list was RSS-shaped.
        let m = MemInfo::parse(FIXTURE);
        let b = MemoryBreakdown::from_meminfo(&m);
        let gib = 1024.0 * 1024.0;
        let used_gib = b.used_real as f64 / gib;
        let cache_gib = b.cache_reclaimable as f64 / gib;
        assert!((used_gib - 10.30).abs() < 0.02, "used {used_gib}");
        assert!((cache_gib - 19.5).abs() < 0.05, "cache {cache_gib}");
        let pct = b.used_fraction() * 100.0;
        assert!((pct - 31.4).abs() < 0.3, "pct {pct}");
    }

    #[test]
    fn unattributed_closes_the_books() {
        let m = MemInfo::parse(FIXTURE);
        let b = MemoryBreakdown::from_meminfo(&m);
        let apps = 3_400_000; // the reported ~3.4 GB process sum
        let un = b.unattributed(apps);
        assert_eq!(b.kernel + apps + un, b.used_real);
    }

    #[test]
    fn malformed_input_does_not_panic() {
        assert_eq!(MemInfo::parse("").mem_total, 0);
        assert_eq!(MemInfo::parse("garbage line without colon").mem_total, 0);
        assert_eq!(MemInfo::parse("MemTotal: notanumber kB").mem_total, 0);
        assert_eq!(MemInfo::parse("MemTotal:\n").mem_total, 0);
        let b = MemoryBreakdown::from_meminfo(&MemInfo::default());
        assert_eq!(b.total, 0);
        assert_eq!(b.used_fraction(), 0.0);
    }
}
