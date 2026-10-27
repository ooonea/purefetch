//! Packages: installed counts per manager, fastfetch-style,
//! e.g. "2422 (dpkg), 1 (flatpak)" or "291 (brew), 13 (brew-cask)". Only
//! managers with count>0. Single Row. Read on-disk layouts where practical;
//! RPM, XBPS and Nix use bounded native queries.
use crate::detect::{Row, Rows};
use std::collections::HashSet;

#[cfg(target_os = "macos")]
pub fn detect() -> Rows {
    // Homebrew keeps one directory per installed formula (Cellar) or cask
    // (Caskroom); $HOMEBREW_PREFIX overrides the per-arch default prefix.
    let prefix = std::env::var("HOMEBREW_PREFIX")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_arch = "aarch64") {
                "/opt/homebrew".to_string()
            } else {
                "/usr/local".to_string()
            }
        });

    let mut parts: Vec<String> = Vec::new();
    for (dir, label) in [
        (format!("{prefix}/Cellar"), "brew"),
        (format!("{prefix}/Caskroom"), "brew-cask"),
        // MacPorts: one directory per installed port.
        ("/opt/local/var/macports/software".to_string(), "macports"),
    ] {
        let n = dir_count(&dir);
        if n > 0 {
            parts.push(format!("{n} ({label})"));
        }
    }
    add_nix(&mut parts);
    if parts.is_empty() {
        return Vec::new();
    }
    vec![Row::val(parts.join(", "))]
}

/// Non-hidden subdirectories of `path` (0 if it does not exist).
fn dir_count(path: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .count()
}

#[cfg(target_os = "linux")]
pub fn detect() -> Rows {
    let mut parts: Vec<String> = Vec::new();

    let dpkg = dpkg_count();
    if dpkg > 0 {
        parts.push(format!("{dpkg} (dpkg)"));
    }

    for (n, label) in [
        (rpm_count(), "rpm"),
        (dir_count("/var/lib/pacman/local"), "pacman"),
        (apk_count(), "apk"),
        (emerge_count(), "emerge"),
        (xbps_count(), "xbps"),
    ] {
        if n > 0 {
            parts.push(format!("{n} ({label})"));
        }
    }

    let flatpak = flatpak_count();
    if flatpak > 0 {
        parts.push(format!("{flatpak} (flatpak)"));
    }

    let snap = snap_count();
    if snap > 0 {
        parts.push(format!("{snap} (snap)"));
    }

    add_nix(&mut parts);
    if parts.is_empty() {
        return Vec::new();
    }
    vec![Row::val(parts.join(", "))]
}

/// Count dpkg stanzas whose status line is exactly "install ok installed".
/// Pure file read of /var/lib/dpkg/status, no subprocess.
#[cfg(target_os = "linux")]
fn dpkg_count() -> usize {
    let Ok(status) = std::fs::read_to_string("/var/lib/dpkg/status") else {
        return 0;
    };
    status
        .lines()
        .filter(|l| *l == "Status: install ok installed")
        .count()
}

/// Count unique flatpak application ids across the system and user roots.
/// Each subdirectory of an app/ root is one app id.
#[cfg(target_os = "linux")]
fn flatpak_count() -> usize {
    let mut ids: HashSet<String> = HashSet::new();
    let mut roots: Vec<String> = vec!["/var/lib/flatpak/app".to_string()];
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            roots.push(format!("{home}/.local/share/flatpak/app"));
        }
    }
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                // Each app id is a directory named after the id.
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(name) = entry.file_name().into_string() {
                        ids.insert(name);
                    }
                }
            }
        }
    }
    ids.len()
}

/// Count /snap/<name>/current symlinks, excluding the "bin" entry.
#[cfg(target_os = "linux")]
fn snap_count() -> usize {
    let Ok(entries) = std::fs::read_dir("/snap") else {
        return 0;
    };
    let mut n = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == std::ffi::OsStr::new("bin") {
            continue;
        }
        // A snap is present when <name>/current resolves.
        let mut current = entry.path();
        current.push("current");
        if current.symlink_metadata().is_ok() {
            n += 1;
        }
    }
    n
}

#[cfg(target_os = "linux")]
fn rpm_count() -> usize {
    crate::util::cmd("rpm", &["-qa", "--queryformat", "%{NAME}\n"])
        .map(|s| {
            s.lines()
                .filter(|l| !l.is_empty() && *l != "gpg-pubkey")
                .count()
        })
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn apk_count() -> usize {
    std::fs::read_to_string("/lib/apk/db/installed")
        .map(|s| s.lines().filter(|l| l.starts_with("P:")).count())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn emerge_count() -> usize {
    std::fs::read_dir("/var/db/pkg")
        .map(|entries| {
            entries
                .flatten()
                .map(|e| {
                    if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        dir_count(&e.path().to_string_lossy())
                    } else {
                        0
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn xbps_count() -> usize {
    crate::util::cmd("xbps-query", &["-l"])
        .map(|s| s.lines().filter(|l| l.starts_with("ii ")).count())
        .unwrap_or(0)
}

fn add_nix(parts: &mut Vec<String>) {
    let mut profiles = vec![
        ("/run/current-system".to_string(), "nix-system"),
        ("/nix/var/nix/profiles/default".to_string(), "nix-default"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            profiles.push((format!("{home}/.nix-profile"), "nix-user"));
            let state = std::env::var("XDG_STATE_HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("{home}/.local/state"));
            profiles.push((format!("{state}/nix/profile"), "nix-user"));
        }
    }
    let mut seen = HashSet::new();
    for (profile, label) in profiles {
        let Ok(path) = std::fs::canonicalize(profile) else {
            continue;
        };
        if !seen.insert(path.clone()) {
            continue;
        }
        let Some(path) = path.to_str() else { continue };
        if let Some(output) = crate::util::cmd("nix-store", &["--query", "--requisites", path]) {
            let n = output.lines().filter(|p| nix_package(p)).count();
            if n > 0 {
                parts.push(format!("{n} ({label})"));
            }
        }
    }
}

// Count versioned closure outputs, excluding auxiliary outputs and the system wrapper.
fn nix_package(path: &str) -> bool {
    let Some(name) = path
        .rsplit('/')
        .next()
        .and_then(|s| s.split_once('-'))
        .map(|(_, n)| n)
    else {
        return false;
    };
    !name.starts_with("nixos-system-")
        && !["-doc", "-man", "-info", "-dev", "-bin"]
            .iter()
            .any(|s| name.ends_with(s))
        && name
            .as_bytes()
            .windows(3)
            .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nix_closure_filters_auxiliary_outputs_and_system_wrappers() {
        for name in ["hello-2.12.1", "linux-7.2.11", "gcc-15.3.0-lib"] {
            assert!(nix_package(&format!("/nix/store/hash-{name}")));
        }
        for name in [
            "nixos-system-unicorn-26.05.1",
            "hello-2.12.1-doc",
            "hello-2.12.1-dev",
            "system-path",
            "source",
        ] {
            assert!(!nix_package(&format!("/nix/store/hash-{name}")));
        }
    }
}
