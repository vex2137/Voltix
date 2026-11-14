use std::fs;
use std::path::PathBuf;

/// Lightweight, non-blocking Linux desktop dark/light theme detection.
/// Checks KDE Plasma (`kdeglobals`) and `GTK_THEME` without spawning subprocesses.
pub fn is_dark_theme_preferred() -> bool {
    if let Ok(gtk_theme) = std::env::var("GTK_THEME") {
        let lower = gtk_theme.to_ascii_lowercase();
        if lower.contains("dark") {
            return true;
        }
        if lower.contains("light") {
            return false;
        }
    }

    if let Some(kde_path) = kdeglobals_path() {
        if let Ok(content) = fs::read_to_string(kde_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(scheme) = trimmed.strip_prefix("ColorScheme=") {
                    let lower = scheme.to_ascii_lowercase();
                    if lower.contains("dark") {
                        return true;
                    }
                    if lower.contains("light") {
                        return false;
                    }
                }
            }
        }
    }

    true
}

fn kdeglobals_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg);
        if !p.as_os_str().is_empty() {
            return Some(p.join("kdeglobals"));
        }
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".config").join("kdeglobals"))
}
