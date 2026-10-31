//! Open-port collection: listening TCP and bound UDP sockets from `/proc/net/*`, mapped to
//! the owning process by inode through `/proc/<pid>/fd`.

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListeningSocket {
    /// `tcp`, `tcp6`, `udp` or `udp6`.
    pub proto: String,
    pub addr: String,
    pub port: u16,
    /// `LISTEN` for TCP, `UNCONN` for bound UDP.
    pub state: String,
    pub inode: u64,
    pub pid: Option<i32>,
    pub comm: Option<String>,
    /// Working directory of the owning process (`/proc/<pid>/cwd`), when readable.
    pub cwd: Option<String>,
}

impl ListeningSocket {
    pub fn endpoint(&self) -> String {
        if self.addr.contains(':') {
            format!("[{}]:{}", self.addr, self.port)
        } else {
            format!("{}:{}", self.addr, self.port)
        }
    }

    /// True for wildcard binds (`0.0.0.0` / `::`), i.e. exposed on all interfaces.
    pub fn is_wildcard(&self) -> bool {
        self.addr == "0.0.0.0" || self.addr == "::"
    }
}

/// One row parsed out of `/proc/net/{tcp,tcp6,udp,udp6}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSocket {
    pub addr: String,
    pub port: u16,
    pub state_hex: String,
    pub inode: u64,
}

pub fn parse_ipv4(hex: &str) -> Option<String> {
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(
        Ipv4Addr::new(
            (v & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            ((v >> 16) & 0xff) as u8,
            ((v >> 24) & 0xff) as u8,
        )
        .to_string(),
    )
}

pub fn parse_ipv6(hex: &str) -> Option<String> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for i in 0..4 {
        let word = u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).ok()?;
        bytes[i * 4..i * 4 + 4].copy_from_slice(&[
            word as u8,
            (word >> 8) as u8,
            (word >> 16) as u8,
            (word >> 24) as u8,
        ]);
    }
    Some(Ipv6Addr::from(bytes).to_string())
}

/// Parse `<hex-address>:<hex-port>` into a display address and port.
pub fn parse_addr(s: &str, v6: bool) -> Option<(String, u16)> {
    let (a, p) = s.split_once(':')?;
    let port = u16::from_str_radix(p, 16).ok()?;
    let addr = if v6 {
        parse_ipv6(a).unwrap_or_else(|| a.to_string())
    } else {
        parse_ipv4(a).unwrap_or_else(|| a.to_string())
    };
    Some((addr, port))
}

pub fn state_label(state_hex: &str) -> &'static str {
    match state_hex {
        "0A" => "LISTEN",
        "07" => "UNCONN",
        "01" => "ESTAB",
        "02" => "SYN_SENT",
        "03" => "SYN_RECV",
        "08" => "CLOSE_WAIT",
        _ => "—",
    }
}

/// Parse a `/proc/net/*` table, keeping only sockets that are actually "open" (TCP LISTEN,
/// UDP UNCONN). `v6` selects the address formatter.
pub fn parse_table(text: &str, v6: bool) -> Vec<RawSocket> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let state_hex = f[3];
        // TCP: 0A LISTEN. UDP: 07 unconnected/bound. Everything else is a live connection.
        if state_hex != "0A" && state_hex != "07" {
            continue;
        }
        let Some((addr, port)) = parse_addr(f[1], v6) else {
            continue;
        };
        out.push(RawSocket {
            addr,
            port,
            state_hex: state_hex.to_string(),
            inode: f[9].parse().unwrap_or(0),
        });
    }
    out
}

/// Build `inode → (pid, comm, cwd)` by walking `/proc/<pid>/fd`. Only inodes in `wanted` are
/// kept. `cwd` is the process's working directory (where it was launched from / chdir'd to).
pub fn map_inodes(wanted: &HashSet<u64>) -> HashMap<u64, (i32, String, Option<String>)> {
    let mut out = HashMap::new();
    if wanted.is_empty() {
        return out;
    }
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return out;
    };
    for ent in rd.flatten() {
        let Some(pid) = ent.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let Some(s) = target.to_str() else { continue };
            let Some(rest) = s.strip_prefix("socket:[") else {
                continue;
            };
            let Some(inode) = rest.strip_suffix(']').and_then(|x| x.parse::<u64>().ok()) else {
                continue;
            };
            if wanted.contains(&inode) && !out.contains_key(&inode) {
                let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .map(|c| c.trim().to_string())
                    .unwrap_or_else(|_| "?".to_string());
                let cwd = std::fs::read_link(format!("/proc/{pid}/cwd"))
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned());
                out.insert(inode, (pid, comm, cwd));
            }
        }
    }
    out
}

/// Rate-limited collector. Scanning every pid's fd table is expensive, so we refresh at most
/// once every few seconds; the port list changes far less often than CPU does.
#[derive(Debug)]
pub struct SocketCollector {
    last: Option<Instant>,
    interval: Duration,
    cached: Vec<ListeningSocket>,
}

impl Default for SocketCollector {
    fn default() -> Self {
        SocketCollector {
            last: None,
            interval: Duration::from_secs(3),
            cached: Vec::new(),
        }
    }
}

impl SocketCollector {
    pub fn sample(&mut self) -> Vec<ListeningSocket> {
        let now = Instant::now();
        let due = self
            .last
            .map(|t| now.duration_since(t) >= self.interval)
            .unwrap_or(true);
        if due {
            self.last = Some(now);
            self.cached = collect();
        }
        self.cached.clone()
    }
}

fn collect() -> Vec<ListeningSocket> {
    let tables = [
        ("/proc/net/tcp", "tcp", false),
        ("/proc/net/tcp6", "tcp6", true),
        ("/proc/net/udp", "udp", false),
        ("/proc/net/udp6", "udp6", true),
    ];
    let mut raw: Vec<(String, RawSocket)> = Vec::new();
    let mut wanted = HashSet::new();
    for (path, proto, v6) in tables {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for s in parse_table(&text, v6) {
            wanted.insert(s.inode);
            raw.push((proto.to_string(), s));
        }
    }
    let owners = map_inodes(&wanted);
    let mut out: Vec<ListeningSocket> = raw
        .into_iter()
        .map(|(proto, s)| {
            let owner = owners.get(&s.inode);
            ListeningSocket {
                proto,
                addr: s.addr,
                port: s.port,
                state: state_label(&s.state_hex).to_string(),
                inode: s.inode,
                pid: owner.map(|(pid, _, _)| *pid),
                comm: owner.map(|(_, comm, _)| comm.clone()),
                cwd: owner.and_then(|(_, _, cwd)| cwd.clone()),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.port
            .cmp(&b.port)
            .then_with(|| a.proto.cmp(&b.proto))
            .then_with(|| a.addr.cmp(&b.addr))
    });
    out.dedup_by(|a, b| {
        a.proto == b.proto && a.port == b.port && a.addr == b.addr && a.inode == b.inode
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_is_little_endian() {
        // 0100007F == 127.0.0.1, 00000000 == 0.0.0.0
        assert_eq!(parse_ipv4("0100007F").as_deref(), Some("127.0.0.1"));
        assert_eq!(parse_ipv4("00000000").as_deref(), Some("0.0.0.0"));
        assert!(parse_ipv4("zzzz").is_none());
    }

    #[test]
    fn ipv6_parses_and_compresses() {
        // ::  (all zero)
        assert_eq!(parse_ipv6(&"0".repeat(32)).as_deref(), Some("::"));
        // ::1
        assert_eq!(
            parse_ipv6("00000000000000000000000001000000").as_deref(),
            Some("::1")
        );
        assert!(parse_ipv6("1234").is_none());
    }

    #[test]
    fn addr_and_port() {
        assert_eq!(
            parse_addr("0100007F:1F90", false),
            Some(("127.0.0.1".into(), 8080))
        );
        assert_eq!(
            parse_addr("00000000:0050", false),
            Some(("0.0.0.0".into(), 80))
        );
    }

    #[test]
    fn parses_only_open_sockets() {
        let text = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1
   1: 0100007F:C350 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 12346 1
   2: 00000000:0035 00000000:0000 07 00000000:00000000 00:00000000 00000000     0        0 999 2
";
        let rows = parse_table(text, false);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].port, 8080);
        assert_eq!(rows[0].state_hex, "0A");
        assert_eq!(rows[1].port, 53);
        assert_eq!(rows[1].state_hex, "07");
    }

    #[test]
    fn state_labels() {
        assert_eq!(state_label("0A"), "LISTEN");
        assert_eq!(state_label("07"), "UNCONN");
        assert_eq!(state_label("FF"), "—");
    }

    #[test]
    fn empty_or_garbage_tables_are_safe() {
        assert!(parse_table("", false).is_empty());
        assert!(parse_table("header only\n", false).is_empty());
        assert!(parse_table("0: garbage\n", false).is_empty());
        assert!(map_inodes(&HashSet::new()).is_empty());
    }

    #[test]
    fn collector_returns_without_panicking() {
        let mut c = SocketCollector::default();
        let a = c.sample();
        let b = c.sample(); // cached path
        assert_eq!(a.len(), b.len());
    }
}
