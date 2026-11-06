//! Application state for the TUI (plain struct, no framework).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use pulsar_core::actions::ActionResult;
use pulsar_core::config::Config;
use pulsar_core::group::Group;
use pulsar_core::snapshot::SystemSnapshot;
use pulsar_core::sockets::ListeningSocket;
use pulsar_core::theme::Theme;

use crate::theme_map::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Processes,
    Memory,
    Power,
    Ports,
}

impl View {
    pub fn all() -> [View; 5] {
        [
            View::Overview,
            View::Processes,
            View::Memory,
            View::Power,
            View::Ports,
        ]
    }
    pub fn label(self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Processes => "Processes",
            View::Memory => "Memory",
            View::Power => "Power",
            View::Ports => "Ports",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Group,
    Flat,
    Tree,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Group => "group",
            ViewMode::Flat => "flat",
            ViewMode::Tree => "tree",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Cpu,
    Pss,
    Io,
    Power,
    Pid,
}

impl SortKey {
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Cpu => "CPU",
            SortKey::Pss => "Memory",
            SortKey::Io => "Disk",
            SortKey::Power => "Power",
            SortKey::Pid => "PID",
        }
    }
    pub fn all() -> [SortKey; 6] {
        [
            SortKey::Pss,
            SortKey::Cpu,
            SortKey::Name,
            SortKey::Io,
            SortKey::Power,
            SortKey::Pid,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Activity,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
    KillGroup,
    FlushSwap,
    DropCaches,
    DropCachesAll,
    Compact,
    EnablePower,
}

#[derive(Debug, Clone)]
pub struct Pending {
    pub kind: PendingKind,
    pub label: String,
    pub detail: String,
    pub pids: Vec<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct Cli {
    pub dump_dir: Option<PathBuf>,
    pub png: bool,
    pub theme: Option<String>,
    pub view: Option<View>,
    pub once: bool,
}

pub static CLI: OnceLock<Cli> = OnceLock::new();

pub struct AppState {
    pub config: Config,
    pub theme: Theme,
    pub palette: Palette,
    pub snapshot: SystemSnapshot,
    pub view: View,
    pub view_mode: ViewMode,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub search: String,
    pub search_active: bool,
    pub expanded: HashSet<String>,
    pub pinned: Vec<String>,
    pub hide_kernel: bool,
    pub selected: Option<String>,
    pub ports_selected: usize,
    pub activity: Vec<ActionResult>,
    pub confirm: Option<Pending>,
    pub overlay: Option<Overlay>,
    pub message: Option<(String, Instant)>,
    pub should_quit: bool,
    order: Vec<String>,
    order_deadline: Instant,
    order_params: (SortKey, bool, ViewMode, bool),
}

impl AppState {
    pub fn new() -> Self {
        let cli = CLI.get();
        let mut config = Config::load();
        if let Some(t) = cli.and_then(|c| c.theme.clone()) {
            config.theme = t;
        }
        let theme = Theme::resolve(&config.theme);
        let palette = Palette::from_theme(&theme);
        let view = cli.and_then(|c| c.view).unwrap_or(View::Overview);
        let hide_kernel = config.hide_kernel_threads;
        AppState {
            config,
            theme,
            palette,
            snapshot: SystemSnapshot::default(),
            view,
            view_mode: ViewMode::Group,
            sort: SortKey::Pss,
            sort_desc: true,
            search: String::new(),
            search_active: false,
            expanded: HashSet::new(),
            pinned: Vec::new(),
            hide_kernel,
            selected: None,
            ports_selected: 0,
            activity: Vec::new(),
            confirm: None,
            overlay: None,
            message: None,
            should_quit: false,
            order: Vec::new(),
            order_deadline: Instant::now(),
            order_params: (SortKey::Pss, true, ViewMode::Group, false),
        }
    }

    pub fn set_message(&mut self, m: impl Into<String>) {
        self.message = Some((m.into(), Instant::now()));
    }

    pub fn current_message(&self) -> Option<&str> {
        self.message
            .as_ref()
            .filter(|(_, t)| t.elapsed().as_secs() < 6)
            .map(|(m, _)| m.as_str())
    }

    pub fn reload_config(&mut self) {
        let cfg = Config::load();
        self.theme = Theme::resolve(&cfg.theme);
        self.palette = Palette::from_theme(&self.theme);
        self.hide_kernel = cfg.hide_kernel_threads;
        self.config = cfg;
        self.set_message("config reloaded");
    }

    pub fn log(&mut self, r: ActionResult) {
        self.set_message(format!("{}: {}", r.action, r.message));
        self.activity.push(r);
        if self.activity.len() > 500 {
            self.activity.remove(0);
        }
    }

    pub fn toggle_expanded(&mut self, key: &str) {
        if !self.expanded.remove(key) {
            self.expanded.insert(key.to_string());
        }
    }

    pub fn toggle_pin(&mut self, key: &str) {
        if let Some(pos) = self.pinned.iter().position(|k| k == key) {
            self.pinned.remove(pos);
        } else {
            self.pinned.push(key.to_string());
        }
    }

    pub fn is_pinned(&self, key: &str) -> bool {
        self.pinned.iter().any(|k| k == key)
    }

    /// The selected group, if any.
    pub fn selected_group(&self) -> Option<&Group> {
        let key = self.selected.as_ref()?;
        self.snapshot.groups.iter().find(|g| &g.key == key)
    }

    pub fn force_reorder(&mut self) {
        self.order_deadline = Instant::now() - std::time::Duration::from_millis(1);
    }

    fn params(&self) -> (SortKey, bool, ViewMode, bool) {
        (self.sort, self.sort_desc, self.view_mode, self.hide_kernel)
    }

    /// Ordered group keys, frozen for 1 s after a change so rows do not jump under the
    /// cursor.
    pub fn order_for(&mut self, groups: &[Group]) -> Vec<String> {
        let params = self.params();
        let now = Instant::now();
        if params != self.order_params || now >= self.order_deadline || self.order.is_empty() {
            let mut keys: Vec<String> = groups.iter().map(|g| g.key.clone()).collect();
            keys.sort_by(|a, b| self.compare_keys(a, b, groups));
            if self.sort_desc {
                keys.reverse();
            }
            self.order = keys.clone();
            self.order_deadline = now + std::time::Duration::from_millis(1000);
            self.order_params = params;
            keys
        } else {
            let mut out: Vec<String> = self
                .order
                .iter()
                .filter(|k| groups.iter().any(|g| &g.key == *k))
                .cloned()
                .collect();
            for g in groups {
                if !out.contains(&g.key) {
                    out.push(g.key.clone());
                }
            }
            out
        }
    }

    fn compare_keys(&self, a: &str, b: &str, groups: &[Group]) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let (Some(ga), Some(gb)) = (
            groups.iter().find(|g| g.key == a),
            groups.iter().find(|g| g.key == b),
        ) else {
            return a.cmp(b);
        };
        let ord = match self.sort {
            SortKey::Name => ga.name.to_lowercase().cmp(&gb.name.to_lowercase()),
            SortKey::Cpu => ga
                .cpu_pct
                .partial_cmp(&gb.cpu_pct)
                .unwrap_or(Ordering::Equal),
            SortKey::Pss => ga.pss.cmp(&gb.pss),
            SortKey::Io => (ga.io_read + ga.io_write).cmp(&(gb.io_read + gb.io_write)),
            SortKey::Power => ga
                .power_w
                .partial_cmp(&gb.power_w)
                .unwrap_or(Ordering::Equal),
            SortKey::Pid => ga.pids.first().cmp(&gb.pids.first()),
        };
        ord.then_with(|| a.cmp(b))
    }

    /// True if a socket row matches the current search filter.
    pub fn port_matches(&self, s: &ListeningSocket) -> bool {
        let q = self.search.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        s.proto.to_lowercase().contains(&q)
            || s.addr.to_lowercase().contains(&q)
            || s.port.to_string().contains(&q)
            || s.comm
                .as_deref()
                .map(|c| c.to_lowercase().contains(&q))
                .unwrap_or(false)
            || s.cwd
                .as_deref()
                .map(|c| c.to_lowercase().contains(&q))
                .unwrap_or(false)
    }

    pub fn port_count(&self) -> usize {
        self.snapshot
            .sockets
            .iter()
            .filter(|s| self.port_matches(s))
            .count()
    }

    /// Move the ports-list selection, clamped to the filtered list.
    pub fn move_port_selection(&mut self, delta: i32) {
        let count = self.port_count();
        if count == 0 {
            self.ports_selected = 0;
            return;
        }
        let cur = self.ports_selected.min(count - 1) as i32;
        self.ports_selected = (cur + delta).rem_euclid(count as i32) as usize;
    }

    pub fn passes_filter(&self, g: &Group, snapshot: &SystemSnapshot) -> bool {
        if self.hide_kernel && g.is_kernel {
            return false;
        }
        let q = self.search.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        if g.name.to_lowercase().contains(&q) {
            return true;
        }
        g.pids.iter().any(|pid| {
            snapshot
                .process_by_pid(*pid)
                .map(|p| {
                    p.comm.to_lowercase().contains(&q)
                        || p.cmdline
                            .as_deref()
                            .map(|c| c.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .unwrap_or(false)
        })
    }

    /// Visible group keys in display order (respecting filter), for navigation.
    pub fn visible_keys(&mut self) -> Vec<String> {
        let groups = self.snapshot.groups.clone();
        let order = self.order_for(&groups);
        let snap = self.snapshot.clone();
        order
            .into_iter()
            .filter(|k| {
                groups
                    .iter()
                    .find(|g| &g.key == k)
                    .map(|g| self.passes_filter(g, &snap))
                    .unwrap_or(false)
            })
            .collect()
    }
}
