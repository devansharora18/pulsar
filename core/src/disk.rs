//! Disk IO counters from `/proc/diskstats` and cheap rate derivation.

use serde::{Deserialize, Serialize};

/// One device's row from `/proc/diskstats`, minus the fields we do not use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskStat {
    pub name: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_bps: f64,
    pub write_bps: f64,
}

/// Parse `/proc/diskstats`. `sectors` are 512 bytes.
pub fn parse_diskstats(text: &str) -> Vec<(String, u64, u64)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 14 {
            continue;
        }
        let name = f[2].to_string();
        let sectors_read: u64 = f[5].parse().unwrap_or(0);
        let sectors_written: u64 = f[9].parse().unwrap_or(0);
        out.push((name, sectors_read * 512, sectors_written * 512));
    }
    out
}

/// Filter to devices worth showing: drop loop/ram pseudo-devices.
pub fn is_interesting_device(name: &str) -> bool {
    !(name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram"))
}

#[derive(Debug, Default)]
pub struct DiskCollector {
    prev: Vec<(String, u64, u64)>,
}

impl DiskCollector {
    pub fn sample(&mut self, dt: f64) -> Vec<DiskStat> {
        let text = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
        let rows = parse_diskstats(&text);
        let mut out = Vec::new();
        for (name, r, w) in &rows {
            if !is_interesting_device(name) {
                continue;
            }
            let (rb, wb) = self
                .prev
                .iter()
                .find(|p| p.0 == *name)
                .map(|(_, pr, pw)| (crate::net::rate(*pr, *r, dt), crate::net::rate(*pw, *w, dt)))
                .unwrap_or((0.0, 0.0));
            out.push(DiskStat {
                name: name.clone(),
                read_bytes: *r,
                write_bytes: *w,
                read_bps: rb,
                write_bps: wb,
            });
        }
        self.prev = rows.iter().map(|(n, r, w)| (n.clone(), *r, *w)).collect();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
   8       0 sda 1234 100 5000 400 2000 200 8000 600 0 0 0 0 0 0
 259       0 nvme0n1 5000 0 20000 100 100 0 4000 50 0 0 0 0 0 0
   7       0 loop0 1 0 1 0 0 0 0 0 0 0 0 0 0 0
";

    #[test]
    fn parses_and_converts_sectors_to_bytes() {
        let rows = parse_diskstats(SAMPLE);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, "sda");
        assert_eq!(rows[0].1, 5000 * 512);
        assert_eq!(rows[0].2, 8000 * 512);
        assert_eq!(rows[1].0, "nvme0n1");
    }

    #[test]
    fn filters_pseudo_devices() {
        assert!(!is_interesting_device("loop0"));
        assert!(!is_interesting_device("zram0"));
        assert!(is_interesting_device("nvme0n1"));
        assert!(is_interesting_device("dm-0"));
    }

    #[test]
    fn short_lines_are_skipped() {
        assert!(parse_diskstats("8 0 sda incomplete\n").is_empty());
    }
}
