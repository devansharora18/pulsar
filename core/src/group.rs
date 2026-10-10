//! Process grouping — the Windows-Task-Manager-style folding.
//!
//! Resolution order (first hit wins):
//! 1. systemd user cgroup `app-*.scope` / `app-*.service` → application id.
//! 2. `.desktop` match on that id, or on the executable basename → friendly name + icon.
//! 3. executable basename (so two `opencode` processes become one row).
//! 4. process-tree root.
//!
//! On top of that, browser helper processes are folded under their browser root.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::desktop::DesktopIndex;
use crate::process::IoCounters;
use crate::pss::ProcMemory;

/// One process as collected for a single tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcSample {
    pub pid: i32,
    pub ppid: i32,
    pub starttime: u64,
    pub comm: String,
    pub exe: Option<String>,
    pub cmdline: Option<String>,
    /// systemd application id from the cgroup, if any.
    pub app_id: Option<String>,
    pub uid: u32,
    pub threads: i64,
    pub mem: ProcMemory,
    pub io: IoCounters,
    pub cpu_pct: f64,
    pub power_w: f64,
}

impl ProcSample {
    /// A process with no `exe` and no `cmdline` is a kernel thread.
    pub fn is_kernel(&self) -> bool {
        self.exe.is_none() && self.cmdline.is_none()
    }

    pub fn exe_basename(&self) -> String {
        self.exe
            .as_deref()
            .and_then(|p| p.rsplit('/').next())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.comm.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GroupKind {
    /// Identified by an application id or desktop entry.
    #[default]
    App,
    /// Identified by executable basename.
    Exe,
    /// Identified by process-tree root.
    Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupMeta {
    pub name: String,
    pub icon: Option<String>,
    pub kind: GroupKind,
}

/// Final per-pid assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assigned {
    pub pid: i32,
    pub key: String,
    pub meta: GroupMeta,
}

/// The aggregated view the UI renders (one expandable row).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub key: String,
    pub name: String,
    pub icon: Option<String>,
    pub kind: GroupKind,
    pub pids: Vec<i32>,
    pub cpu_pct: f64,
    pub pss: u64,
    pub rss: u64,
    pub uss: u64,
    pub swap: u64,
    pub io_read: u64,
    pub io_write: u64,
    pub power_w: f64,
    pub is_kernel: bool,
    /// Rolling power history for the row sparkline.
    #[serde(default)]
    pub spark: Vec<f64>,
}

impl Group {
    pub fn count(&self) -> usize {
        self.pids.len()
    }
    /// "opencode (2)" style label.
    pub fn label(&self) -> String {
        if self.pids.len() > 1 {
            format!("{} ({})", self.name, self.pids.len())
        } else {
            self.name.clone()
        }
    }
}

const BROWSER_ROOTS: &[&str] = &[
    "zen",
    "zen-bin",
    "firefox",
    "firefox-bin",
    "chrome",
    "google-chrome",
    "chromium",
    "chromium-browser",
    "brave",
    "brave-browser",
    "msedge",
    "microsoft-edge",
    "vivaldi",
    "librewolf",
    "floorp",
];

const BROWSER_CHILD_COMMS: &[&str] = &[
    "Web Content",
    "WebExtensions",
    "Privileged Content",
    "Privileged Cont",
    "RDD Process",
    "Socket Process",
    "GPU Process",
    "Isolated Web Co",
    "Isolated Web Content",
    "Isolated Service",
    "Utility Process",
    "Renderer",
    "PPAPI Process",
    "Chrome_ChildIOT",
    "Chrome_IOThread",
    "gpu-process",
    "utility",
    "tab",
    "socket",
    "rdd",
    "forkserver",
];

fn is_browser_root_basename(base: &str) -> bool {
    BROWSER_ROOTS.iter().any(|b| b.eq_ignore_ascii_case(base))
}

/// Resolve the base group key + metadata for a single process, ignoring browser folding.
pub fn base_key(s: &ProcSample, index: &DesktopIndex) -> (String, GroupMeta) {
    // 1 + 2: cgroup application id.
    if let Some(app_id) = &s.app_id {
        if let Some(e) = index.match_app_id(app_id) {
            return (
                app_id.clone(),
                GroupMeta {
                    name: e.name.clone(),
                    icon: e.icon.clone(),
                    kind: GroupKind::App,
                },
            );
        }
        return (
            app_id.clone(),
            GroupMeta {
                name: pretty_app_id(app_id),
                icon: None,
                kind: GroupKind::App,
            },
        );
    }
    // 2b: desktop entry by executable basename.
    let base = s.exe_basename();
    if let Some(e) = index.match_exe(&base) {
        return (
            base.clone(),
            GroupMeta {
                name: e.name.clone(),
                icon: e.icon.clone(),
                kind: GroupKind::App,
            },
        );
    }
    // 3: executable basename.
    (
        base.clone(),
        GroupMeta {
            name: base,
            icon: None,
            kind: GroupKind::Exe,
        },
    )
}

/// Assign every process to a group key, applying browser-child folding.
pub fn assign_groups(samples: &[ProcSample], index: &DesktopIndex) -> Vec<Assigned> {
    let by_pid: HashMap<i32, &ProcSample> = samples.iter().map(|s| (s.pid, s)).collect();
    let mut resolved: HashMap<i32, (String, GroupMeta)> = HashMap::new();
    for s in samples {
        resolved.insert(s.pid, base_key(s, index));
    }

    let mut out = Vec::with_capacity(samples.len());
    for s in samples {
        let (mut key, mut meta) = resolved
            .get(&s.pid)
            .cloned()
            .unwrap_or_else(|| base_key(s, index));

        if is_browser_child(s) {
            if let Some(root) = browser_root_ancestor(s, &by_pid) {
                if let Some((rk, rm)) = resolved.get(&root) {
                    key = rk.clone();
                    meta = rm.clone();
                }
            }
        }

        out.push(Assigned {
            pid: s.pid,
            key,
            meta,
        });
    }
    out
}

fn is_browser_child(s: &ProcSample) -> bool {
    if BROWSER_CHILD_COMMS
        .iter()
        .any(|c| c.eq_ignore_ascii_case(&s.comm))
    {
        return true;
    }
    // A browser-root exe that is not the topmost of its name (i.e. a child helper).
    let base = s.exe_basename();
    is_browser_root_basename(&base) && s.ppid > 1
}

/// Walk ancestors to the nearest one whose exe is a known browser root.
fn browser_root_ancestor(s: &ProcSample, by_pid: &HashMap<i32, &ProcSample>) -> Option<i32> {
    let mut cur = s.ppid;
    let mut guard = 0;
    while cur > 1 && guard < 64 {
        guard += 1;
        let Some(parent) = by_pid.get(&cur) else {
            break;
        };
        if is_browser_root_basename(&parent.exe_basename()) {
            return Some(parent.pid);
        }
        cur = parent.ppid;
    }
    None
}

/// Aggregate assigned processes into groups.
pub fn build_groups(assigned: &[Assigned], samples: &[ProcSample]) -> Vec<Group> {
    let by_pid: HashMap<i32, &ProcSample> = samples.iter().map(|s| (s.pid, s)).collect();
    let mut map: HashMap<String, Group> = HashMap::new();
    for a in assigned {
        let Some(s) = by_pid.get(&a.pid) else {
            continue;
        };
        let g = map.entry(a.key.clone()).or_insert_with(|| Group {
            key: a.key.clone(),
            name: a.meta.name.clone(),
            icon: a.meta.icon.clone(),
            kind: a.meta.kind,
            is_kernel: false,
            ..Default::default()
        });
        g.pids.push(s.pid);
        g.cpu_pct += s.cpu_pct;
        g.pss = g.pss.saturating_add(s.mem.pss);
        g.rss = g.rss.saturating_add(s.mem.rss);
        g.uss = g.uss.saturating_add(s.mem.uss);
        g.swap = g.swap.saturating_add(s.mem.swap);
        g.io_read = g.io_read.saturating_add(s.io.read_bytes);
        g.io_write = g.io_write.saturating_add(s.io.write_bytes);
        g.power_w += s.power_w;
        if s.is_kernel() {
            g.is_kernel = true;
        }
    }
    let mut groups: Vec<Group> = map.into_values().collect();
    for g in &mut groups {
        g.pids.sort_unstable();
    }
    groups
}

/// Turn a machine app id into a readable fallback name: `org.gnome.Nautilus` → `Nautilus`.
fn pretty_app_id(id: &str) -> String {
    id.rsplit('.').next().unwrap_or(id).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::DesktopEntry;

    fn sample(
        pid: i32,
        ppid: i32,
        comm: &str,
        exe: Option<&str>,
        app_id: Option<&str>,
    ) -> ProcSample {
        ProcSample {
            pid,
            ppid,
            starttime: pid as u64,
            comm: comm.to_string(),
            exe: exe.map(str::to_string),
            cmdline: exe.map(|e| e.to_string()),
            app_id: app_id.map(str::to_string),
            uid: 1000,
            threads: 1,
            mem: ProcMemory {
                pss: 100,
                rss: 200,
                uss: 50,
                shared: 100,
                swap: 0,
                from_rollup: true,
            },
            io: IoCounters::default(),
            cpu_pct: 1.0,
            power_w: 0.1,
        }
    }

    fn index() -> DesktopIndex {
        let zen = "[Desktop Entry]\nName=Zen Browser\nExec=/usr/bin/flatpak run app.zen_browser.zen\nIcon=app.zen_browser.zen\nStartupWMClass=zen\nType=Application\n";
        let ff = "[Desktop Entry]\nName=Firefox\nExec=/usr/lib64/firefox/firefox %u\nIcon=firefox\nType=Application\n";
        DesktopIndex {
            entries: vec![
                DesktopEntry::parse(zen, "app.zen_browser.zen", std::path::Path::new("t")).unwrap(),
                DesktopEntry::parse(ff, "firefox", std::path::Path::new("t")).unwrap(),
            ],
        }
    }

    #[test]
    fn zen_children_fold_into_one_app_group() {
        let s = vec![
            sample(
                100,
                1,
                "zen",
                Some("/app/zen/zen"),
                Some("app.zen_browser.zen"),
            ),
            sample(
                101,
                100,
                "Web Content",
                Some("/app/zen/zen"),
                Some("app.zen_browser.zen"),
            ),
            sample(
                102,
                100,
                "Isolated Web Co",
                Some("/app/zen/zen"),
                Some("app.zen_browser.zen"),
            ),
        ];
        let idx = index();
        let assigned = assign_groups(&s, &idx);
        assert!(assigned.iter().all(|a| a.key == "app.zen_browser.zen"));
        let groups = build_groups(&assigned, &s);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Zen Browser");
        assert_eq!(groups[0].count(), 3);
    }

    #[test]
    fn two_opencode_processes_become_one_row() {
        let s = vec![
            sample(
                200,
                1,
                "opencode",
                Some("/home/u/.opencode/bin/opencode"),
                None,
            ),
            sample(
                201,
                1,
                "opencode",
                Some("/home/u/.opencode/bin/opencode"),
                None,
            ),
        ];
        let assigned = assign_groups(&s, &DesktopIndex::default());
        let groups = build_groups(&assigned, &s);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "opencode");
        assert_eq!(groups[0].label(), "opencode (2)");
    }

    #[test]
    fn firefox_helpers_with_distinct_exe_fold_to_root() {
        // A synthetic child whose exe basename is NOT the browser root, but whose parent is.
        let mut child = sample(
            302,
            301,
            "Web Content",
            Some("/usr/lib64/firefox/helper"),
            None,
        );
        child.cmdline = Some("helper".into());
        let s = vec![
            sample(301, 1, "firefox", Some("/usr/lib64/firefox/firefox"), None),
            child,
        ];
        let assigned = assign_groups(&s, &DesktopIndex::default());
        assert_eq!(assigned[0].key, "firefox");
        assert_eq!(assigned[1].key, "firefox");
    }

    #[test]
    fn unrelated_pythons_group_by_exe_not_by_cgroup() {
        let s = vec![
            sample(401, 1, "python3", Some("/usr/bin/python3"), None),
            sample(402, 1, "python3", Some("/usr/bin/python3"), None),
        ];
        let assigned = assign_groups(&s, &DesktopIndex::default());
        let groups = build_groups(&assigned, &s);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "python3");
        assert_eq!(groups[0].count(), 2);
    }

    #[test]
    fn kernel_threads_are_flagged_and_can_be_filtered() {
        let t = ProcSample {
            pid: 9,
            ppid: 2,
            starttime: 9,
            comm: "kworker/0:0".into(),
            exe: None,
            cmdline: None,
            app_id: None,
            uid: 0,
            threads: 1,
            mem: ProcMemory::default(),
            io: IoCounters::default(),
            cpu_pct: 0.0,
            power_w: 0.0,
        };
        assert!(t.is_kernel());
        let s = vec![t];
        let assigned = assign_groups(&s, &DesktopIndex::default());
        let groups = build_groups(&assigned, &s);
        assert!(groups[0].is_kernel);
    }

    #[test]
    fn group_aggregates_sums() {
        let s = vec![
            sample(500, 1, "opencode", Some("/x/opencode"), None),
            sample(501, 1, "opencode", Some("/x/opencode"), None),
        ];
        let assigned = assign_groups(&s, &DesktopIndex::default());
        let g = &build_groups(&assigned, &s)[0];
        assert_eq!(g.pss, 200);
        assert_eq!(g.rss, 400);
        assert!((g.cpu_pct - 2.0).abs() < 1e-9);
        assert!((g.power_w - 0.2).abs() < 1e-9);
    }
}
