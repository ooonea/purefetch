//! DE: desktop environment + version, e.g. "GNOME 48.7".
//! Name from $XDG_CURRENT_DESKTOP (colon list / "ubuntu:GNOME" -> last token,
//! normalized), falling back to $XDG_SESSION_DESKTOP then $DESKTOP_SESSION.
//! Version comes from the DE's own tool (gnome-shell/plasmashell); if unknown
//! only the name is shown. Empty Vec on a tty / pure WM (no DE hints).
use crate::detect::{Row, Rows};

pub fn detect() -> Rows {
    let raw = desktop_hint();
    let Some(raw) = raw else {
        return Vec::new();
    };
    // A colon-separated list such as "ubuntu:GNOME": take the last non-empty token.
    let token = raw.split(':').map(str::trim).rfind(|t| !t.is_empty());
    let Some(token) = token else {
        return Vec::new();
    };
    let name = normalize(token);
    let value = match version_for(&name) {
        Some(v) => format!("{name} {v}"),
        None => name,
    };
    vec![Row::val(value)]
}

/// First of the desktop env vars that is set and non-empty.
fn desktop_hint() -> Option<String> {
    for key in [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Map a raw desktop token to a canonical display name; unknown tokens pass
/// through unchanged.
fn normalize(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    let name = if lower.contains("gnome") {
        "GNOME"
    } else if lower.contains("kde") || lower.contains("plasma") {
        "KDE Plasma"
    } else if lower.contains("xfce") {
        "XFCE"
    } else if lower.contains("cinnamon") {
        "Cinnamon"
    } else if lower.contains("mate") {
        "MATE"
    } else if lower.contains("lxqt") {
        "LXQt"
    } else if lower.contains("lxde") {
        "LXDE"
    } else if lower.contains("budgie") {
        "Budgie"
    } else if lower.contains("deepin") {
        "Deepin"
    } else if lower.contains("pantheon") {
        "Pantheon"
    } else if lower.contains("enlightenment") {
        "Enlightenment"
    } else if lower.contains("unity") {
        "Unity"
    } else {
        return token.to_string();
    };
    name.to_string()
}

/// DE version string, or None if it cannot be determined.
fn version_for(name: &str) -> Option<String> {
    match name {
        "GNOME" => {
            crate::util::cmd("gnome-shell", &["--version"]).and_then(|s| first_num_token(&s))
        }
        // Prefer the full Plasma version from plasmashell; the KDE_SESSION_VERSION
        // env var only carries the major (e.g. "6"), used only as a fallback.
        "KDE Plasma" => crate::util::cmd("plasmashell", &["--version"])
            .and_then(|s| first_num_token(&s))
            .or_else(|| match std::env::var("KDE_SESSION_VERSION") {
                Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
                _ => None,
            }),
        "XFCE" => {
            crate::util::cmd("xfce4-session", &["--version"]).and_then(|s| first_num_token(&s))
        }
        "MATE" => {
            crate::util::cmd("mate-session", &["--version"]).and_then(|s| first_num_token(&s))
        }
        "Cinnamon" => {
            crate::util::cmd("cinnamon", &["--version"]).and_then(|s| first_num_token(&s))
        }
        _ => None,
    }
}

/// First whitespace-separated token that begins with a digit,
/// e.g. "GNOME Shell 48.7" -> "48.7".
fn first_num_token(s: &str) -> Option<String> {
    s.split_whitespace()
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|t| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::{first_num_token, normalize};

    #[test]
    fn normalize_canonicalizes_known_desktops() {
        assert_eq!(normalize("GNOME"), "GNOME");
        assert_eq!(normalize("KDE"), "KDE Plasma");
        assert_eq!(normalize("plasma"), "KDE Plasma");
        assert_eq!(normalize("xfce"), "XFCE");
        assert_eq!(normalize("SomethingElse"), "SomethingElse");
    }

    #[test]
    fn first_num_token_extracts_version() {
        assert_eq!(first_num_token("GNOME Shell 48.7").as_deref(), Some("48.7"));
        assert_eq!(
            first_num_token("plasmashell 6.2.4").as_deref(),
            Some("6.2.4")
        );
        assert_eq!(first_num_token("no version"), None);
    }
}
