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

    /// Optional Nerd Font glyph for the tab bar.
    pub fn icon(self) -> &'static str {
        match self {
            View::Overview => "◱",
            View::Processes => "≡",
            View::Memory => "▦",
            View::Power => "⚡",
            View::Ports => "⇄",
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

/// Rows of the settings panel, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Theme,
    Icons,
    Refresh,
    Smoothing,
    Tdp,
    History,
    HideKernel,
    Confirm,
    Save,
    Reload,
    WriteTheme,
    Reset,
}

pub const SETTINGS: [Setting; 12] = [
    Setting::Theme,
    Setting::Icons,
    Setting::Refresh,
    Setting::Smoothing,
    Setting::Tdp,
    Setting::History,
    Setting::HideKernel,
    Setting::Confirm,
    Setting::Save,
    Setting::Reload,
    Setting::WriteTheme,
    Setting::Reset,
];

/// Built-in themes plus any user `.toml` files in the theme directory.
pub fn theme_names() -> Vec<String> {
    let mut names: Vec<String> = Theme::builtin_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    if let Ok(rd) = std::fs::read_dir(Theme::theme_dir()) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !names.iter().any(|n| n == stem) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names
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
    pub ports_grouped: bool,
    pub ports_expanded: HashSet<String>,
    pub activity: Vec<ActionResult>,
    pub confirm: Option<Pending>,
    pub overlay: Option<Overlay>,
    pub settings_cursor: usize,
    pub control: Option<pulsar_core::sampler::SamplerControl>,
    pub message: Option<(String, Instant)>,
    pub should_quit: bool,
    /// Smoothed headline values (EMA, 1 Hz) so bars do not jitter.
    pub cpu_ema: f64,
    pub power_ema: f64,
    pub core_ema: Vec<f64>,
    /// Rolling per-core usage for the tile sparklines.
    pub core_hist: Vec<Vec<f64>>,
    last_seq: u64,
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
            ports_grouped: true,
            ports_expanded: HashSet::new(),
            activity: Vec::new(),
            confirm: None,
            overlay: None,
            settings_cursor: 0,
            control: None,
            message: None,
            should_quit: false,
            cpu_ema: 0.0,
            power_ema: 0.0,
            core_ema: Vec::new(),
            core_hist: Vec::new(),
            last_seq: 0,
            order: Vec::new(),
            order_deadline: Instant::now(),
            order_params: (SortKey::Pss, true, ViewMode::Group, false),
        }
    }

    /// Fold a new snapshot into the UI state: EMA headline values and per-core history.
    pub fn ingest(&mut self, snap: SystemSnapshot) {
        if snap.seq != self.last_seq {
            let n = snap.cpu.cores.len();
            if self.core_ema.len() != n {
                self.core_ema = vec![0.0; n];
                self.core_hist = vec![Vec::new(); n];
            }
            let first = self.last_seq == 0;
            let alpha = self.config.ema_alpha();
            let ema = |prev: f64, v: f64| {
                if first {
                    v
                } else {
                    prev + alpha * (v - prev)
                }
            };
            self.cpu_ema = ema(self.cpu_ema, snap.cpu.overall_usage_pct);
            self.power_ema = ema(self.power_ema, snap.power.total_w);
            for (i, c) in snap.cpu.cores.iter().enumerate() {
                self.core_ema[i] = ema(self.core_ema[i], c.usage_pct);
                let h = &mut self.core_hist[i];
                h.push(c.usage_pct);
                if h.len() > 30 {
                    h.remove(0);
                }
            }
            self.last_seq = snap.seq;
        }
        self.snapshot = snap;
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
        if let Some(c) = &self.control {
            c.set_refresh_ms(cfg.refresh_ms);
            c.set_tdp_w(cfg.tdp_w);
        }
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

    /// Sockets grouped by owning process (pid + comm), or one `unowned` bucket.
    pub fn port_groups(&self) -> Vec<PortGroup<'_>> {
        let mut map: std::collections::HashMap<String, PortGroup<'_>> =
            std::collections::HashMap::new();
        for s in self
            .snapshot
            .sockets
            .iter()
            .filter(|s| self.port_matches(s))
        {
            let key = match s.pid {
                Some(pid) => format!("{pid}:{}", s.comm.clone().unwrap_or_default()),
                None => "unowned".to_string(),
            };
            let g = map.entry(key.clone()).or_insert_with(|| PortGroup {
                key,
                name: s.comm.clone().unwrap_or_else(|| "unowned".to_string()),
                pid: s.pid,
                sockets: Vec::new(),
                cwd: None,
                all_ifaces: 0,
                iface: 0,
                localhost: 0,
            });
            g.sockets.push(s);
            if g.cwd.is_none() {
                g.cwd = s.cwd.clone();
            }
            if s.is_wildcard() {
                g.all_ifaces += 1;
            } else if s.addr == "127.0.0.1" || s.addr == "::1" {
                g.localhost += 1;
            } else {
                g.iface += 1;
            }
        }
        let mut groups: Vec<PortGroup<'_>> = map.into_values().collect();
        groups.sort_by(|a, b| b.count().cmp(&a.count()).then_with(|| a.name.cmp(&b.name)));
        groups
    }

    pub fn port_row_count(&self) -> usize {
        if self.ports_grouped {
            self.port_groups().len()
        } else {
            self.port_count()
        }
    }

    pub fn selected_port_group(&self) -> Option<PortGroup<'_>> {
        let groups = self.port_groups();
        if groups.is_empty() {
            return None;
        }
        let i = self.ports_selected.min(groups.len() - 1);
        Some(groups.into_iter().nth(i).expect("index in range"))
    }

    pub fn toggle_port_group(&mut self) {
        if let Some(g) = self.selected_port_group() {
            let key = g.key.clone();
            if !self.ports_expanded.remove(&key) {
                self.ports_expanded.insert(key);
            }
        }
    }

    /// The socket currently selected in the Ports view (filtered + sorted like the table).
    pub fn selected_port(&self) -> Option<&ListeningSocket> {
        let mut idx: Vec<usize> = (0..self.snapshot.sockets.len())
            .filter(|i| self.port_matches(&self.snapshot.sockets[*i]))
            .collect();
        if idx.is_empty() {
            return None;
        }
        idx.sort_by_key(|i| self.snapshot.sockets[*i].port);
        let i = *idx.get(self.ports_selected.min(idx.len() - 1))?;
        self.snapshot.sockets.get(i)
    }

    /// Move the ports-list selection, clamped to the filtered list.
    pub fn move_port_selection(&mut self, delta: i32) {
        let count = self.port_row_count();
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

/// One row of the grouped Ports view: a process and the sockets it owns.
pub struct PortGroup<'a> {
    pub key: String,
    pub name: String,
    pub pid: Option<i32>,
    pub sockets: Vec<&'a ListeningSocket>,
    pub cwd: Option<String>,
    pub all_ifaces: usize,
    pub iface: usize,
    pub localhost: usize,
}

impl PortGroup<'_> {
    pub fn count(&self) -> usize {
        self.sockets.len()
    }

    pub fn label(&self) -> String {
        match self.pid {
            Some(_) if self.count() > 1 => format!("{} ×{}", self.name, self.count()),
            Some(_) => self.name.clone(),
            None => format!("unowned ×{}", self.count()),
        }
    }

    pub fn ports(&self, max: usize) -> String {
        let mut ports: Vec<u16> = self.sockets.iter().map(|s| s.port).collect();
        ports.sort_unstable();
        ports.dedup();
        let joined = ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        truncate(&joined, max)
    }

    pub fn scopes(&self) -> String {
        // Dominant exposure wins; the per-socket detail is in the expanded rows.
        if self.all_ifaces > 0 {
            "all ifaces".to_string()
        } else if self.iface > 0 {
            "interface".to_string()
        } else {
            "localhost".to_string()
        }
    }

    pub fn pids(&self) -> Vec<i32> {
        let mut pids: Vec<i32> = self.sockets.iter().filter_map(|s| s.pid).collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    if n <= 1 {
        return "…".to_string();
    }
    let mut out: String = s.chars().take(n - 1).collect();
    out.push('…');
    out
}
