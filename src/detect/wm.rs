//! WM: window manager / compositor, e.g. "Mutter (Wayland)".
//!
//! Prefer the current desktop identity, then known compositor ancestors.
//! The session type ($XDG_SESSION_TYPE) is appended as " (Wayland)"/" (X11)".
use crate::detect::{Row, Rows};

/// Standalone WMs/compositors, matched on the exact /proc/<pid>/comm name
/// (comm is truncated to 15 chars by the kernel, so every entry stays short).
/// Left = comm as it appears in /proc; right = pretty display name.
/// The exact match deliberately avoids helper processes such as
/// "mutter-x11-fram" (mutter-x11-frames) that merely share a prefix.
#[cfg(target_os = "linux")]
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

/// macOS: the compositor is always WindowServer, presented under its
/// canonical name (fastfetch/neofetch convention). Tiling add-ons (yabai,
/// AeroSpace, ...) still run on top of it, so this stays honest either way.
#[cfg(target_os = "macos")]
pub fn detect() -> Rows {
    vec![Row::val("Quartz Compositor")]
}

#[cfg(target_os = "linux")]
pub fn detect() -> Rows {
    let wm = match de_to_wm().or_else(wm_ancestor) {
        Some(name) => name,
        None => return Vec::new(),
    };
    let value = match session_suffix() {
        Some(sess) => format!("{wm} ({sess})"),
        None => wm.to_string(),
    };
    vec![Row::val(value)]
}

#[cfg(target_os = "linux")]
fn wm_ancestor() -> Option<&'static str> {
    let mut pid = crate::sys::ppid_comm(std::process::id())?.0;
    for _ in 0..64 {
        if pid <= 1 {
            break;
        }
        let (parent, comm) = crate::sys::ppid_comm(pid)?;
        if let Some((_, name)) = KNOWN_WMS.iter().find(|(name, _)| *name == comm) {
            return Some(name);
        }
        if parent == pid {
            break;
        }
        pid = parent;
    }
    None
}

/// Map the current desktop environment to its default WM/compositor.
/// $XDG_CURRENT_DESKTOP may be a colon-separated list (e.g. "ubuntu:GNOME").
#[cfg(target_os = "linux")]
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
            _ => match KNOWN_WMS
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&de))
            {
                Some((_, name)) => name,
                None => continue,
            },
        };
        return Some(wm);
    }
    None
}

/// Pretty session type from $XDG_SESSION_TYPE, or None if not X11/Wayland.
#[cfg(target_os = "linux")]
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
