//! `/proc/<pid>/cgroup` → application identity.
//!
//! On a systemd+cgroup-v2 desktop the most reliable "this is one application" signal is
//! the scope/service systemd launched it in. GNOME and flatpak both do this.

/// Extract a stable application id from a unified cgroup path, if the process was started
/// as a desktop application. Returns `None` for terminal spawns, system daemons, etc., so
/// the caller can fall back to exe-based grouping.
///
/// Handles the forms observed on this box:
/// - `app-flatpak-app.zen_browser.zen-2878664422.scope` → `app.zen_browser.zen`
/// - `app-gnome-org.gnome.Nautilus-1234.scope`         → `org.gnome.Nautilus`
/// - `app-zen_browser.zen-2878664422.scope`            → `zen_browser.zen`
/// - `app-code-1234.scope`                             → `code`
///
/// And deliberately ignores `ptyxis-spawn-*.scope` (a terminal, not an app).
pub fn app_id_from_cgroup_path(path: &str) -> Option<String> {
    for seg in path.split('/') {
        let stem = match seg
            .strip_suffix(".scope")
            .or_else(|| seg.strip_suffix(".service"))
        {
            Some(s) => s,
            None => continue,
        };
        let Some(rest) = stem.strip_prefix("app-") else {
            continue;
        };
        let rest = rest.strip_prefix("flatpak-").unwrap_or(rest);
        let rest = rest.strip_prefix("gnome-").unwrap_or(rest);
        let id = strip_trailing_numeric_suffix(rest);
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    None
}

/// Read `/proc/<pid>/cgroup` and return the unified path (`0::<path>`), if present.
pub fn unified_cgroup_path(pid: i32) -> Option<String> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    for line in text.lines() {
        let mut it = line.splitn(3, ':');
        let _hierarchy = it.next()?;
        let controllers = it.next()?;
        let path = it.next()?;
        if controllers.is_empty() {
            return Some(path.to_string());
        }
    }
    // v1 fallback: take any path.
    text.lines()
        .next()
        .and_then(|l| l.splitn(3, ':').nth(2))
        .map(str::to_string)
}

/// From the unified path, resolve the desktop application id.
pub fn app_id_for_pid(pid: i32) -> Option<String> {
    let path = unified_cgroup_path(pid)?;
    let is_app = path.contains("/app-") || path.contains(".scope") || path.contains(".service");
    if !is_app {
        return None;
    }
    app_id_from_cgroup_path(&path)
}

fn strip_trailing_numeric_suffix(s: &str) -> &str {
    if let Some((head, tail)) = s.rsplit_once('-') {
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
            return head;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatpak_zen_scope_resolves_to_dotted_app_id() {
        let p = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-graphical.slice/app-flatpak-app.zen_browser.zen-2878664422.scope";
        assert_eq!(
            app_id_from_cgroup_path(p).as_deref(),
            Some("app.zen_browser.zen")
        );
    }

    #[test]
    fn gnome_scope_resolves_to_desktop_id() {
        let p = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-org.gnome.Nautilus-2628.scope";
        assert_eq!(
            app_id_from_cgroup_path(p).as_deref(),
            Some("org.gnome.Nautilus")
        );
    }

    #[test]
    fn plain_named_scope_is_used() {
        let p = "/user.slice/app.slice/app-code-4242.scope";
        assert_eq!(app_id_from_cgroup_path(p).as_deref(), Some("code"));
    }

    #[test]
    fn terminal_spawn_is_not_an_application() {
        let p = "/user.slice/user-1000.slice/user@1000.service/app.slice/ptyxis-spawn-e6b38cd2-df2e-466b-8f7e-5e109d666a68.scope";
        assert_eq!(app_id_from_cgroup_path(p), None);
    }

    #[test]
    fn service_with_at_suffix_keeps_identity() {
        let p = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-org.gnome.gedit@autostart.service";
        assert_eq!(
            app_id_from_cgroup_path(p).as_deref(),
            Some("org.gnome.gedit@autostart")
        );
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(app_id_from_cgroup_path(""), None);
        assert_eq!(app_id_from_cgroup_path("/system.slice/sshd.service"), None);
    }
}
