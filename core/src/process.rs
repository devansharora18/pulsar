//! `/proc/<pid>/stat`, `/proc/<pid>/status`, `/proc/<pid>/statm` and friends.

use std::io::Read;

use serde::{Deserialize, Serialize};

use crate::Result;

/// Clock ticks per second (`sysconf(_SC_CLK_TCK)`), needed to turn jiffies into time.
pub fn clock_ticks_per_sec() -> u64 {
    // SAFETY: sysconf is a pure query.
    let v = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if v > 0 {
        v as u64
    } else {
        100
    }
}

/// Page size in bytes (`sysconf(_SC_PAGESIZE)`).
pub fn page_size() -> u64 {
    // SAFETY: sysconf is a pure query.
    let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if v > 0 {
        v as u64
    } else {
        4096
    }
}

/// Fields we use out of `/proc/<pid>/stat`. See `proc(5)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stat {
    pub pid: i32,
    pub comm: String,
    pub state: char,
    pub ppid: i32,
    pub utime: u64,
    pub stime: u64,
    pub cutime: i64,
    pub cstime: i64,
    pub nice: i32,
    pub num_threads: i64,
    /// Jiffies since boot when the process started. Combined with boot time this gives a
    /// monotonic identity that survives PID reuse.
    pub starttime: u64,
    pub vsize_bytes: u64,
    pub rss_pages: i64,
}

impl Stat {
    pub fn parse(text: &str) -> Result<Self> {
        // comm is wrapped in the first '(' and the *last* ')' because it may itself
        // contain spaces and parentheses.
        let open = text.find('(').ok_or_else(|| crate::Error::Parse("stat: no (".into()))?;
        let close = text.rfind(')').ok_or_else(|| crate::Error::Parse("stat: no )".into()))?;
        if close < open {
            return Err(crate::Error::Parse("stat: bad parens".into()));
        }
        let pid: i32 = text[..open].trim().parse().map_err(|_| crate::Error::Parse("stat: pid".into()))?;
        let comm = text[open + 1..close].to_string();
        // Fields after state: ppid is index 1 (state is index 0).
        let rest: Vec<&str> = text[close + 1..].split_whitespace().collect();
        let get = |i: usize| -> Option<&str> { rest.get(i).copied() };
        let state = get(0).and_then(|s| s.chars().next()).unwrap_or('?');
        let parse_u = |i: usize| -> u64 { get(i).and_then(|s| s.parse().ok()).unwrap_or(0) };
        let parse_i = |i: usize| -> i64 { get(i).and_then(|s| s.parse().ok()).unwrap_or(0) };
        Ok(Stat {
            pid,
            comm,
            state,
            ppid: parse_i(1) as i32,
            utime: parse_u(11),
            stime: parse_u(12),
            cutime: parse_i(13),
            cstime: parse_i(14),
            nice: parse_i(16) as i32,
            num_threads: parse_i(17),
            starttime: parse_u(19),
            vsize_bytes: parse_u(20),
            rss_pages: parse_i(21),
        })
    }

    /// Total CPU time in jiffies (user + system), excluding dead children.
    pub fn total_ticks(&self) -> u64 {
        self.utime.saturating_add(self.stime)
    }

    pub fn read(pid: i32) -> Result<Self> {
        let mut s = String::new();
        std::fs::File::open(format!("/proc/{pid}/stat"))?.read_to_string(&mut s)?;
        Self::parse(&s)
    }
}

/// The handful of `/proc/<pid>/status` fields we keep. All sizes in kibibytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub name: String,
    pub state: char,
    pub uid: u32,
    pub gid: u32,
    pub vmrss: u64,
    pub vmhwm: u64,
    pub vmswap: u64,
    pub threads: i64,
}

impl Status {
    pub fn parse(text: &str) -> Self {
        let mut s = Status::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else { continue };
            let v = v.trim();
            match k {
                "Name" => s.name = v.to_string(),
                "State" => s.state = v.chars().next().unwrap_or('?'),
                "Uid" => s.uid = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0),
                "Gid" => s.gid = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0),
                "VmRSS" => s.vmrss = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0),
                "VmHWM" => s.vmhwm = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0),
                "VmSwap" => s.vmswap = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0),
                "Threads" => s.threads = v.parse().unwrap_or(0),
                _ => {}
            }
        }
        s
    }

    pub fn read(pid: i32) -> Result<Self> {
        let mut s = String::new();
        std::fs::File::open(format!("/proc/{pid}/status"))?.read_to_string(&mut s)?;
        Ok(Self::parse(&s))
    }
}

/// `/proc/<pid>/statm` in pages: size resident shared text lib data dt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Statm {
    pub size: u64,
    pub resident: u64,
    pub shared: u64,
}

impl Statm {
    pub fn parse(text: &str) -> Option<Self> {
        let mut it = text.split_whitespace();
        let size = it.next()?.parse().ok()?;
        let resident = it.next()?.parse().ok()?;
        let shared = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        Some(Statm { size, resident, shared })
    }

    pub fn rss_kb(&self) -> u64 {
        self.resident.saturating_mul(page_size()) / 1024
    }

    pub fn read(pid: i32) -> Option<Self> {
        Self::parse(&std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?)
    }
}

/// `/proc/<pid>/cmdline` as a single space-joined string.
pub fn read_cmdline(pid: i32) -> Option<String> {
    let bytes = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let s: String = bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Resolved `/proc/<pid>/exe` path.
pub fn read_exe(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// `/proc/<pid>/io` — real disk bytes. Network is deliberately absent (Linux does not
/// expose per-pid network counters), so Pulsar never fakes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoCounters {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

impl IoCounters {
    pub fn parse(text: &str) -> Self {
        let mut io = IoCounters::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else { continue };
            let v = v.trim().parse().unwrap_or(0);
            match k {
                "read_bytes" => io.read_bytes = v,
                "write_bytes" => io.write_bytes = v,
                _ => {}
            }
        }
        io
    }

    pub fn read(pid: i32) -> Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(format!("/proc/{pid}/io"))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real /proc/1/stat shape; comm contains spaces and parens to exercise the parser.
    const STAT: &str = "1 (init (weird) name) S 0 1 1 0 -1 4194560 1234 567 0 0 42 7 3 4 20 0 1 0 21003 237809664 948 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0";

    #[test]
    fn stat_parses_comm_with_spaces_and_parens() {
        let s = Stat::parse(STAT).unwrap();
        assert_eq!(s.pid, 1);
        assert_eq!(s.comm, "init (weird) name");
        assert_eq!(s.state, 'S');
        assert_eq!(s.ppid, 0);
        assert_eq!(s.utime, 42);
        assert_eq!(s.stime, 7);
        assert_eq!(s.cutime, 3);
        assert_eq!(s.cstime, 4);
        assert_eq!(s.nice, 0);
        assert_eq!(s.num_threads, 1);
        assert_eq!(s.starttime, 21003);
        assert_eq!(s.rss_pages, 948);
        assert_eq!(s.total_ticks(), 49);
    }

    #[test]
    fn stat_bad_input_is_error_not_panic() {
        assert!(Stat::parse("garbage").is_err());
        assert!(Stat::parse("(no pid) S").is_err());
    }

    #[test]
    fn status_parses_core_fields() {
        let text = "Name:\tbash\nState:\tS (sleeping)\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\nVmRSS:\t  3844 kB\nVmHWM:\t 10240 kB\nVmSwap:\t 128 kB\nThreads:\t3\n";
        let s = Status::parse(text);
        assert_eq!(s.name, "bash");
        assert_eq!(s.state, 'S');
        assert_eq!(s.uid, 1000);
        assert_eq!(s.vmrss, 3844);
        assert_eq!(s.vmhwm, 10240);
        assert_eq!(s.vmswap, 128);
        assert_eq!(s.threads, 3);
    }

    #[test]
    fn statm_and_io_parse() {
        let st = Statm::parse("58059 961 884 266 0 106 0").unwrap();
        assert_eq!(st.resident, 961);
        let io = IoCounters::parse("rchar: 31469\nwchar: 1119\nread_bytes: 0\nwrite_bytes: 4096\n");
        assert_eq!(io.read_bytes, 0);
        assert_eq!(io.write_bytes, 4096);
        assert_eq!(Statm::parse("").is_none(), true);
    }
}
