//! `/proc/<pid>/smaps_rollup` — the *correct* per-process memory accounting.
//!
//! RSS double-counts shared pages, which is exactly why GNOME's list summed to ~3.4 GB
//! while the header said 10.3 GB. PSS assigns each shared page fractionally to its
//! sharers, so a sum of PSS across processes approaches the system "used" figure.

use serde::{Deserialize, Serialize};

/// All sizes in kibibytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmapsRollup {
    pub rss: u64,
    pub pss: u64,
    pub pss_anon: u64,
    pub pss_file: u64,
    pub pss_shmem: u64,
    pub shared_clean: u64,
    pub shared_dirty: u64,
    pub private_clean: u64,
    pub private_dirty: u64,
    pub swap: u64,
}

impl SmapsRollup {
    pub fn parse(text: &str) -> Self {
        let mut r = SmapsRollup::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else { continue };
            let Some(num) = v.split_whitespace().next().and_then(|x| x.parse::<u64>().ok())
            else {
                continue;
            };
            match k {
                "Rss" => r.rss = num,
                "Pss" => r.pss = num,
                "Pss_Anon" => r.pss_anon = num,
                "Pss_File" => r.pss_file = num,
                "Pss_Shmem" => r.pss_shmem = num,
                "Shared_Clean" => r.shared_clean = num,
                "Shared_Dirty" => r.shared_dirty = num,
                "Private_Clean" => r.private_clean = num,
                "Private_Dirty" => r.private_dirty = num,
                "Swap" => r.swap = num,
                _ => {}
            }
        }
        r
    }

    pub fn read(pid: i32) -> crate::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(format!(
            "/proc/{pid}/smaps_rollup"
        ))?))
    }

    /// Unique Set Size: pages this process alone owns.
    pub fn uss(&self) -> u64 {
        self.private_clean.saturating_add(self.private_dirty)
    }

    /// Shared pages (clean + dirty).
    pub fn shared(&self) -> u64 {
        self.shared_clean.saturating_add(self.shared_dirty)
    }

    /// The number Pulsar shows by default so group sums reconcile with the header.
    pub fn display(&self) -> u64 {
        self.pss
    }
}

/// Per-process memory with an explicit provenance marker, so the UI can show "RSS" when
/// the kernel would not give us a rollup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcMemory {
    pub pss: u64,
    pub rss: u64,
    pub uss: u64,
    pub shared: u64,
    pub swap: u64,
    /// True when `pss`/`uss` came from a rollup; false when we fell back to RSS only.
    pub from_rollup: bool,
}

impl ProcMemory {
    /// Preferred path: `smaps_rollup`. Falls back to RSS (statm/stat) when unreadable, in
    /// which case PSS == RSS and shared/uss are reported as 0 rather than invented.
    pub fn read(pid: i32, rss_kb_fallback: u64, swap_kb: u64) -> Self {
        match SmapsRollup::read(pid) {
            Ok(r) if r.rss > 0 || r.pss > 0 => ProcMemory {
                pss: r.pss,
                rss: r.rss,
                uss: r.uss(),
                shared: r.shared(),
                swap: r.swap,
                from_rollup: true,
            },
            _ => ProcMemory {
                pss: rss_kb_fallback,
                rss: rss_kb_fallback,
                uss: 0,
                shared: 0,
                swap: swap_kb,
                from_rollup: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
559071076000-7fff227cd000 ---p 00000000 00:00 0                          [rollup]
Rss:                3844 kB
Pss:                 479 kB
Pss_Dirty:           308 kB
Pss_Anon:            308 kB
Pss_File:            171 kB
Pss_Shmem:             0 kB
Shared_Clean:       3532 kB
Shared_Dirty:          0 kB
Private_Clean:         4 kB
Private_Dirty:       308 kB
Referenced:         3844 kB
Anonymous:           308 kB
KSM:                   0 kB
Swap:                  0 kB
SwapPss:               0 kB
Locked:                0 kB
";

    #[test]
    fn parses_rollup() {
        let r = SmapsRollup::parse(FIXTURE);
        assert_eq!(r.rss, 3844);
        assert_eq!(r.pss, 479);
        assert_eq!(r.uss(), 312);
        assert_eq!(r.shared(), 3532);
        assert_eq!(r.display(), 479);
    }

    #[test]
    fn falls_back_to_rss_when_rollup_missing() {
        // Reading a nonexistent pid must yield the RSS fallback, never a panic.
        let m = ProcMemory::read(i32::MAX, 1234, 56);
        assert!(!m.from_rollup);
        assert_eq!(m.pss, 1234);
        assert_eq!(m.rss, 1234);
        assert_eq!(m.uss, 0);
        assert_eq!(m.swap, 56);
    }

    #[test]
    fn malformed_rollup_is_zero_not_panic() {
        let r = SmapsRollup::parse("garbage\nRss: notanumber kB\n");
        assert_eq!(r.rss, 0);
        assert_eq!(r.pss, 0);
    }
}
