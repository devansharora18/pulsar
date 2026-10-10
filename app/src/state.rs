//! Shared UI state: signals for the snapshot, view, filters and pending actions.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use dioxus::prelude::*;
use pulsar_core::actions::ActionResult;
use pulsar_core::config::Config;
use pulsar_core::group::Group;
use pulsar_core::snapshot::SystemSnapshot;
use pulsar_core::theme::Theme;

use crate::theme_css::theme_css;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Processes,
    Memory,
    Power,
}

impl View {
    pub fn all() -> [View; 4] {
        [View::Overview, View::Processes, View::Memory, View::Power]
    }
    pub fn label(&self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Processes => "Processes",
            View::Memory => "Memory",
            View::Power => "Power",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            View::Overview => "◱",
            View::Processes => "≡",
            View::Memory => "▦",
            View::Power => "⚡",
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
    pub fn label(&self) -> &'static str {
        match self {
            ViewMode::Group => "Group",
            ViewMode::Flat => "Flat",
            ViewMode::Tree => "Tree",
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
    Threads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawer {
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

#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub kind: PendingKind,
    pub label: String,
    pub detail: String,
    pub pids: Vec<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct Cli {
    pub screenshot: Option<PathBuf>,
    pub theme: Option<String>,
    pub view: Option<View>,
    pub expand: bool,
    pub delay_secs: u64,
}

/// Parsed command line, set once in `main`.
pub static CLI: OnceLock<Cli> = OnceLock::new();

#[derive(Clone, Copy)]
pub struct AppState {
    pub snapshot: Signal<SystemSnapshot>,
    pub config: Signal<Config>,
    pub theme: Signal<Theme>,
    pub view: Signal<View>,
    pub search: Signal<String>,
    pub search_open: Signal<bool>,
    pub sort: Signal<SortKey>,
    pub sort_desc: Signal<bool>,
    pub expanded: Signal<HashSet<String>>,
    pub pinned: Signal<Vec<String>>,
    pub view_mode: Signal<ViewMode>,
    pub hide_kernel: Signal<bool>,
    pub activity: Signal<Vec<ActionResult>>,
    pub drawer: Signal<Option<Drawer>>,
    pub cheat: Signal<bool>,
    pub confirm: Signal<Option<Pending>>,
    pub settings: Signal<bool>,
    pub selected: Signal<Option<String>>,
    /// Frozen sort order (keys) so rows do not jump while the user reads them.
    order: Signal<Vec<String>>,
    order_deadline: Signal<Instant>,
    order_params: Signal<(SortKey, bool, ViewMode, bool)>,
}

impl AppState {
    pub fn new() -> Self {
        let cli = CLI.get();
        let mut config = Config::load();
        if let Some(t) = cli.and_then(|c| c.theme.clone()) {
            config.theme = t;
        }
        let theme = Theme::resolve(&config.theme);
        let hide_kernel = config.hide_kernel_threads;
        let initial_view = cli.and_then(|c| c.view).unwrap_or(View::Overview);
        AppState {
            snapshot: Signal::new(SystemSnapshot::default()),
            config: Signal::new(config),
            theme: Signal::new(theme),
            view: Signal::new(initial_view),
            search: Signal::new(String::new()),
            search_open: Signal::new(false),
            sort: Signal::new(SortKey::Pss),
            sort_desc: Signal::new(true),
            expanded: Signal::new(HashSet::new()),
            pinned: Signal::new(Vec::new()),
            view_mode: Signal::new(ViewMode::Group),
            hide_kernel: Signal::new(hide_kernel),
            activity: Signal::new(Vec::new()),
            drawer: Signal::new(None),
            cheat: Signal::new(false),
            confirm: Signal::new(None),
            settings: Signal::new(false),
            selected: Signal::new(None),
            order: Signal::new(Vec::new()),
            order_deadline: Signal::new(Instant::now()),
            order_params: Signal::new((SortKey::Pss, true, ViewMode::Group, false)),
        }
    }

    pub fn css(&self) -> String {
        let cfg = self.config.read();
        let theme = self.theme.read();
        theme_css(&theme, &cfg)
    }

    pub fn config_snapshot(&self) -> Config {
        self.config.read().clone()
    }

    pub fn theme_snapshot(&self) -> Theme {
        self.theme.read().clone()
    }

    pub fn reload_config(&self) {
        let cfg = Config::load();
        let theme = Theme::resolve(&cfg.theme);
        let mut c = self.config;
        c.set(cfg);
        let mut t = self.theme;
        t.set(theme);
    }

    pub fn log(&self, r: ActionResult) {
        let mut a = self.activity;
        a.write().push(r);
    }

    pub fn toggle_expanded(&self, key: &str) {
        let mut e = self.expanded;
        let mut set = e.write();
        if set.contains(key) {
            set.remove(key);
        } else {
            set.insert(key.to_string());
        }
    }

    pub fn toggle_pin(&self, key: &str) {
        let mut p = self.pinned;
        let mut v = p.write();
        if let Some(pos) = v.iter().position(|k| k == key) {
            v.remove(pos);
        } else {
            v.push(key.to_string());
        }
    }

    /// Force the sort order to be recomputed on the next render (after a header click).
    pub fn force_reorder(&self) {
        let mut d = self.order_deadline;
        d.set(Instant::now() - std::time::Duration::from_millis(1));
    }

    /// Whether the current filter/sort parameters changed the ordering.
    fn params(&self) -> (SortKey, bool, ViewMode, bool) {
        (
            *self.sort.read(),
            *self.sort_desc.read(),
            *self.view_mode.read(),
            *self.hide_kernel.read(),
        )
    }

    /// Return group keys in display order, freezing for 1s after a change so the list is
    /// stable while the user points at it.
    pub fn order_for(&self, groups: &[Group]) -> Vec<String> {
        let params = self.params();
        let now = Instant::now();
        let changed = *self.order_params.read() != params;
        let expired = now >= *self.order_deadline.read();
        if changed || expired || self.order.read().is_empty() {
            let mut keys: Vec<String> = groups.iter().map(|g| g.key.clone()).collect();
            keys.sort_by(|a, b| self.compare_keys(a, b, groups));
            if *self.sort_desc.read() {
                keys.reverse();
            }
            let mut o = self.order;
            o.set(keys.clone());
            let mut d = self.order_deadline;
            d.set(now + std::time::Duration::from_millis(1000));
            let mut p = self.order_params;
            p.set(params);
            keys
        } else {
            // Reuse the frozen order, appending keys we have not seen yet.
            let existing = self.order.read().clone();
            let mut out: Vec<String> = existing
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
        let ga = groups.iter().find(|g| g.key == a);
        let gb = groups.iter().find(|g| g.key == b);
        let (Some(ga), Some(gb)) = (ga, gb) else {
            return a.cmp(b);
        };
        let ord = match *self.sort.read() {
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
            SortKey::Threads => ga.count().cmp(&gb.count()),
        };
        // Stable tie-break so equal rows never swap.
        ord.then_with(|| a.cmp(b))
    }

    /// Filter a group against the current search box and kernel-thread toggle.
    pub fn passes_filter(&self, g: &Group, snapshot: &SystemSnapshot) -> bool {
        if *self.hide_kernel.read() && g.is_kernel {
            return false;
        }
        let q = self.search.read().trim().to_lowercase();
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
