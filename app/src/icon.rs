//! Resolve `.desktop` `Icon=` names to an inlineable image via the active icon theme,
//! falling back to `hicolor`. Icons are embedded as data URIs so the custom protocol does
//! not need filesystem access at render time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;

static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

const SIZES: &[&str] = &["scalable", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24", "256x256"];
const EXTS: &[&str] = &["svg", "png"];

/// Current GTK icon theme, or `None`.
pub fn current_icon_theme() -> Option<String> {
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let name = s.trim().trim_matches('\'').trim_matches('"');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn icon_base_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(theme) = current_icon_theme() {
        dirs.push(PathBuf::from("/usr/share/icons").join(&theme));
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(".local/share/icons").join(&theme));
        }
    }
    dirs.push(PathBuf::from("/usr/share/icons/hicolor"));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/icons/hicolor"));
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/icons/hicolor"));
    }
    dirs.push(PathBuf::from("/usr/share/pixmaps"));
    dirs
}

/// Resolve an icon name to a data URI, cached. `name` may be an absolute path.
pub fn resolve(name: &str) -> Option<String> {
    {
        let c = cache().lock().ok()?;
        if let Some(v) = c.get(name) {
            return v.clone();
        }
    }
    let result = resolve_uncached(name);
    if let Ok(mut c) = cache().lock() {
        c.insert(name.to_string(), result.clone());
    }
    result
}

fn resolve_uncached(name: &str) -> Option<String> {
    let path = if name.starts_with('/') {
        let p = PathBuf::from(name);
        p.exists().then_some(p)
    } else {
        find_icon_file(name)
    }?;
    let bytes = std::fs::read(&path).ok()?;
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    };
    Some(format!("data:{mime};base64,{}", base64(&bytes)))
}

fn find_icon_file(name: &str) -> Option<PathBuf> {
    for dir in icon_base_dirs() {
        for size in SIZES {
            for cat in ["apps", "devices", "categories", "mimetypes"] {
                for ext in EXTS {
                    let candidate = dir.join(size).join(cat).join(format!("{name}.{ext}"));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        // Some themes are flat; try the root too.
        for ext in EXTS {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Minimal base64 encoder (standard alphabet, padded).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// True if a file exists at the resolved icon path (test helper).
pub fn icon_exists(name: &str) -> bool {
    find_icon_file(name).is_some() || Path::new(name).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn resolving_a_nonexistent_icon_is_none_not_panic() {
        assert!(resolve("this-icon-does-not-exist-xyz").is_none());
    }
}
