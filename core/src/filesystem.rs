//! Mounted filesystems and their usage via `statvfs`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filesystem {
    pub device: String,
    pub mount: String,
    pub fstype: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub avail_bytes: u64,
}

impl Filesystem {
    pub fn used_fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes as f64 / self.total_bytes as f64
        }
    }
}

/// Parse `/proc/mounts` into `(device, mount, fstype)` triples.
pub fn parse_mounts(text: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 {
            continue;
        }
        out.push((f[0].to_string(), f[1].to_string(), f[2].to_string()));
    }
    out
}

/// Filesystems worth showing.
pub fn is_interesting_fs(fstype: &str) -> bool {
    !matches!(
        fstype,
        "proc"
            | "sysfs"
            | "devtmpfs"
            | "devpts"
            | "cgroup"
            | "cgroup2"
            | "securityfs"
            | "pstore"
            | "efivarfs"
            | "bpf"
            | "tmpfs"
            | "debugfs"
            | "tracefs"
            | "autofs"
            | "mqueue"
            | "hugetlbfs"
            | "fusectl"
            | "configfs"
            | "binfmt_misc"
            | "rpc_pipefs"
            | "nsfs"
            | "overlay"
            | "squashfs"
    )
}

pub fn read_filesystems() -> Vec<Filesystem> {
    let text = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let mut out = Vec::new();
    for (device, mount, fstype) in parse_mounts(&text) {
        if !is_interesting_fs(&fstype) {
            continue;
        }
        let Some(vfs) = statvfs(&mount) else { continue };
        let total = vfs.blocks.saturating_mul(vfs.fragment_size);
        let free = vfs.blocks_free.saturating_mul(vfs.fragment_size);
        let avail = vfs.blocks_available.saturating_mul(vfs.fragment_size);
        if total == 0 {
            continue;
        }
        out.push(Filesystem {
            device,
            mount,
            fstype,
            total_bytes: total,
            used_bytes: total.saturating_sub(free),
            avail_bytes: avail,
        });
    }
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

struct Vfs {
    blocks: u64,
    blocks_free: u64,
    blocks_available: u64,
    fragment_size: u64,
}

fn statvfs(mount: &str) -> Option<Vfs> {
    let path = std::path::Path::new(mount);
    let st = nix::sys::statvfs::statvfs(path).ok()?;
    Some(Vfs {
        blocks: st.blocks(),
        blocks_free: st.blocks_free(),
        blocks_available: st.blocks_available(),
        fragment_size: st.fragment_size(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mounts() {
        let m = parse_mounts(
            "/dev/nvme0n1p3 / btrfs rw,seclabel,compress=zstd:1 0 0\nproc /proc proc rw 0 0\n",
        );
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].0, "/dev/nvme0n1p3");
        assert_eq!(m[0].2, "btrfs");
    }

    #[test]
    fn filters_pseudo_filesystems() {
        assert!(!is_interesting_fs("proc"));
        assert!(!is_interesting_fs("tmpfs"));
        assert!(is_interesting_fs("btrfs"));
        assert!(is_interesting_fs("ext4"));
    }

    #[test]
    fn malformed_mounts_do_not_panic() {
        assert!(parse_mounts("garbage line\n\n").is_empty());
    }
}
