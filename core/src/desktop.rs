//! Minimal, dependency-free `.desktop` parsing and matching.
//!
//! We only need four keys (`Name`, `Icon`, `Exec`, `StartupWMClass`) to turn an application
//! id or an executable basename into a friendly name and an icon.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopEntry {
    /// File name without the `.desktop` suffix, e.g. `app.zen_browser.zen`.
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub wm_class: Option<String>,
    /// Executable basenames this entry could correspond to.
    pub exec_names: Vec<String>,
    pub path: PathBuf,
}

impl DesktopEntry {
    /// Parse the `[Desktop Entry]` group of a `.desktop` file. Returns `None` for entries
    /// that are hidden, not applications, or lack a name.
    pub fn parse(content: &str, id: &str, path: &Path) -> Option<Self> {
        let mut in_group = false;
        let mut name = None;
        let mut icon = None;
        let mut exec = None;
        let mut wm_class = None;
        let mut hidden = false;
        let mut no_display = false;
        let mut ty = None;
        for raw in content.lines() {
            let line = raw.trim();
            if line.starts_with('[') {
                in_group = line == "[Desktop Entry]";
                continue;
            }
            if !in_group || line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k {
                // Skip localized keys like `Name[es]`.
                "Name" => name = Some(v.to_string()),
                "Icon" => icon = Some(v.to_string()),
                "Exec" => exec = Some(v.to_string()),
                "StartupWMClass" => wm_class = Some(v.to_string()),
                "Hidden" => hidden = v.eq_ignore_ascii_case("true"),
                "NoDisplay" => no_display = v.eq_ignore_ascii_case("true"),
                "Type" => ty = Some(v.to_string()),
                _ => {}
            }
        }
        if hidden || no_display {
            return None;
        }
        if let Some(t) = &ty {
            if !t.eq_ignore_ascii_case("Application") {
                return None;
            }
        }
        let name = name?;
        let exec_names = exec.as_deref().map(exec_candidates).unwrap_or_default();
        Some(DesktopEntry {
            id: id.to_string(),
            name,
            icon,
            exec,
            wm_class,
            exec_names,
            path: path.to_path_buf(),
        })
    }
}

/// Extract plausible executable basenames from an `Exec=` line, stripping field codes and
/// unwrapping flatpak's `flatpak run ... <app-id>` form.
pub fn exec_candidates(exec: &str) -> Vec<String> {
    let tokens: Vec<&str> = exec
        .split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect();
    let mut out = Vec::new();
    if let Some(first) = tokens.first() {
        if let Some(base) = basename(first) {
            out.push(base);
        }
    }
    if out.first().map(|s| s == "flatpak").unwrap_or(false) {
        // `flatpak run --flags app.id` → the token right after `run` that is not a flag.
        if let Some(pos) = tokens.iter().position(|t| *t == "run") {
            if let Some(app) = tokens[pos + 1..].iter().find(|t| !t.starts_with('-')) {
                if let Some(base) = basename(app) {
                    out.push(base);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn basename(p: &str) -> Option<String> {
    let b = p.rsplit('/').next().unwrap_or(p);
    if b.is_empty() {
        None
    } else {
        Some(b.to_string())
    }
}

/// An index of installed desktop entries used to resolve names and icons.
#[derive(Debug, Clone, Default)]
pub struct DesktopIndex {
    pub entries: Vec<DesktopEntry>,
}

impl DesktopIndex {
    pub fn load_default() -> Self {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for base in [
            "/usr/share/applications",
            "/usr/local/share/applications",
            "/var/lib/flatpak/exports/share/applications",
            "/usr/share/applications",
        ] {
            dirs.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(".local/share/applications"));
            dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
        }
        if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
            for d in xdg.split(':') {
                dirs.push(PathBuf::from(d).join("applications"));
            }
        }
        dirs.sort();
        dirs.dedup();
        Self::load_from_dirs(&dirs)
    }

    pub fn load_from_dirs(dirs: &[PathBuf]) -> Self {
        let mut entries = Vec::new();
        // Later directories (user overrides) should win; collect then dedup by id keeping
        // the last occurrence.
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for dir in dirs {
            let Ok(rd) = std::fs::read_dir(dir) else {
                continue;
            };
            for ent in rd.flatten() {
                let path = ent.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                let Some(id) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                else {
                    continue;
                };
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if let Some(e) = DesktopEntry::parse(&content, &id, &path) {
                    if let Some(&i) = seen.get(&e.id) {
                        entries[i] = e;
                    } else {
                        seen.insert(e.id.clone(), entries.len());
                        entries.push(e);
                    }
                }
            }
        }
        DesktopIndex { entries }
    }

    /// Match an application id coming from a cgroup scope or a WM class.
    pub fn match_app_id(&self, app_id: &str) -> Option<&DesktopEntry> {
        let needle = app_id.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| e.id.to_ascii_lowercase() == needle)
            .or_else(|| {
                self.entries.iter().find(|e| {
                    e.wm_class
                        .as_deref()
                        .map(|w| w.eq_ignore_ascii_case(app_id))
                        .unwrap_or(false)
                })
            })
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|e| e.id.to_ascii_lowercase().ends_with(&needle))
            })
    }

    /// Match an executable basename to an entry via `Exec` or `StartupWMClass`.
    pub fn match_exe(&self, exe_basename: &str) -> Option<&DesktopEntry> {
        let needle = exe_basename.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| {
                e.exec_names
                    .iter()
                    .any(|n| n.to_ascii_lowercase() == needle)
            })
            .or_else(|| {
                self.entries.iter().find(|e| {
                    e.wm_class
                        .as_deref()
                        .map(|w| w.to_ascii_lowercase() == needle)
                        .unwrap_or(false)
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZEN: &str = "[Desktop Entry]\nName=Zen Browser\nExec=/usr/bin/flatpak run --branch=stable --command=launch-script.sh app.zen_browser.zen @@u %u @@\nIcon=app.zen_browser.zen\nStartupWMClass=zen\nType=Application\n";
    const FF: &str = "[Desktop Entry]\nName=Firefox\nExec=/usr/lib64/firefox/firefox %u\nIcon=firefox\nType=Application\n";
    const HIDDEN: &str =
        "[Desktop Entry]\nName=Hidden\nExec=foo\nNoDisplay=true\nType=Application\n";
    const LOCALIZED: &str = "[Desktop Entry]\nName=Caja\nName[es]=Caja\nExec=caja\nIcon=system-file-manager\nType=Application\n";

    fn entry(id: &str, content: &str) -> DesktopEntry {
        DesktopEntry::parse(content, id, Path::new("test")).unwrap()
    }

    #[test]
    fn parses_flatpak_exec_and_wmclass() {
        let e = entry("app.zen_browser.zen", ZEN);
        assert_eq!(e.name, "Zen Browser");
        assert_eq!(e.wm_class.as_deref(), Some("zen"));
        assert!(e.exec_names.contains(&"flatpak".to_string()));
        assert!(e.exec_names.contains(&"app.zen_browser.zen".to_string()));
    }

    #[test]
    fn hidden_entries_are_skipped() {
        assert!(DesktopEntry::parse(HIDDEN, "hidden", Path::new("t")).is_none());
    }

    #[test]
    fn localized_names_are_ignored() {
        let e = entry("caja", LOCALIZED);
        assert_eq!(e.name, "Caja");
    }

    #[test]
    fn index_matches_by_app_id_and_exe() {
        let index = DesktopIndex {
            entries: vec![entry("app.zen_browser.zen", ZEN), entry("firefox", FF)],
        };
        assert_eq!(
            index.match_app_id("app.zen_browser.zen").unwrap().name,
            "Zen Browser"
        );
        // flatpak app id via wmclass
        assert_eq!(index.match_app_id("zen").unwrap().name, "Zen Browser");
        assert_eq!(index.match_exe("firefox").unwrap().name, "Firefox");
        assert!(index.match_exe("nonexistent-binary").is_none());
    }
}
