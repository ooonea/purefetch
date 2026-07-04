//! WM: window manager / compositor, e.g. "Mutter (Wayland)".
//!
//! Strategy:
//!  (a) scan running processes (/proc/*/comm) for a known standalone
//!      WM/compositor and use it directly;
//!  (b) otherwise map the current desktop environment ($XDG_CURRENT_DESKTOP)
//!      to its default WM (GNOME's compositor lives inside the gnome-shell
//!      process, so this mapping is the reliable path under GNOME/Wayland).
//! The session type ($XDG_SESSION_TYPE) is appended as " (Wayland)"/" (X11)".
use crate::detect::{Row, Rows};

/// Standalone WMs/compositors, matched on the exact /proc/<pid>/comm name
/// (comm is truncated to 15 chars by the kernel, so every entry stays short).
/// Left = comm as it appears in /proc; right = pretty display name.
/// The exact match deliberately avoids helper processes such as
/// "mutter-x11-fram" (mutter-x11-frames) that merely share a prefix.
const KNOWN_WMS: &[(&str, &str)] = &[
    ("kwin_wayland", "KWin"),
    ("kwin_x11", "KWin"),
    ("Hyprland", "Hyprland"),
    ("hyprland", "Hyprland"),
    ("mutter", "Mutter"),
    ("sway", "Sway"),
    ("weston", "Weston"),
    ("river", "River"),
    ("wayfire", "Wayfire"),
    ("niri", "niri"),
    ("labwc", "labwc"),
    ("i3", "i3"),
    ("bspwm", "bspwm"),
    ("openbox", "Openbox"),
    ("xfwm4", "Xfwm4"),
    ("dwm", "dwm"),
    ("awesome", "awesome"),
    ("qtile", "qtile"),
];

pub fn detect() -> Rows {
    let wm = match scan_processes() {
        Some(name) => name,
        None => match de_to_wm() {
            Some(name) => name,
            None => return Vec::new(),
        },
    };
    let value = match session_suffix() {
        Some(sess) => format!("{wm} ({sess})"),
        None => wm.to_string(),
    };
    vec![Row::val(value)]
}

/// Return the pretty name of the first known standalone WM found running.
fn scan_processes() -> Option<&'static str> {
    let dir = std::fs::read_dir("/proc").ok()?;
    for entry in dir.flatten() {
        // Only numeric names are process directories.
        let name = entry.file_name();
        let Some(pid) = name.to_str() else { continue };
        if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let comm = match crate::util::read_trim(&format!("/proc/{pid}/comm")) {
            Some(c) => c,
            None => continue,
        };
        for (proc_name, pretty) in KNOWN_WMS {
            if comm == *proc_name {
                return Some(pretty);
            }
        }
    }
    None
}

/// Map the current desktop environment to its default WM/compositor.
/// $XDG_CURRENT_DESKTOP may be a colon-separated list (e.g. "ubuntu:GNOME").
fn de_to_wm() -> Option<&'static str> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok()?;
    for token in desktop.split(':') {
        let de = token.trim().to_ascii_uppercase();
        let wm = match de.as_str() {
            "GNOME" => "Mutter",
            "KDE" => "KWin",
            "XFCE" => "Xfwm4",
            "LXQT" => "Openbox",
            "CINNAMON" | "X-CINNAMON" => "Muffin",
            "MATE" => "Marco",
            _ => continue,
        };
        return Some(wm);
    }
    None
}

/// Pretty session type from $XDG_SESSION_TYPE, or None if not X11/Wayland.
fn session_suffix() -> Option<&'static str> {
    match std::env::var("XDG_SESSION_TYPE")
        .ok()?
        .to_ascii_lowercase()
        .as_str()
    {
        "wayland" => Some("Wayland"),
        "x11" => Some("X11"),
        _ => None,
    }
}
