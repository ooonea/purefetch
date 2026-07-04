//! Display: connected outputs and resolutions, e.g. "1920x1080 (eDP-1)".
//! Sourced from /sys/class/drm/cardN-<connector>/{status,modes}.
use crate::detect::{Row, Rows};

pub fn detect() -> Rows {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };

    let mut outputs: Vec<(String, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(dir) = name.to_str() else {
            continue;
        };
        // Only per-connector nodes like "card0-eDP-1" (skip "card0", "version", ...).
        let Some(connector) = strip_card_prefix(dir) else {
            continue;
        };

        let base = format!("/sys/class/drm/{dir}");
        // Keep only outputs the kernel reports as connected.
        if crate::util::read_trim(&format!("{base}/status")).as_deref() != Some("connected") {
            continue;
        }
        // First line of "modes" is the preferred/native resolution.
        let Some(mode) = crate::util::first_line(&format!("{base}/modes")) else {
            continue;
        };
        if mode.is_empty() {
            continue;
        }
        outputs.push((connector.to_string(), mode));
    }

    // read_dir order is unspecified; sort for stable output.
    outputs.sort_by(|a, b| a.0.cmp(&b.0));
    outputs
        .into_iter()
        .map(|(connector, mode)| Row::val(format!("{mode} ({connector})")))
        .collect()
}

/// Strip a leading "cardN-" from a DRM node name: "card0-eDP-1" -> Some("eDP-1").
/// Returns None if the name is not a per-connector node (e.g. "card0", "renderD128").
fn strip_card_prefix(dir: &str) -> Option<&str> {
    let rest = dir.strip_prefix("card")?;
    // Consume the card index digits.
    let after_digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    // Must have consumed at least one digit and be followed by '-<connector>'.
    if after_digits.len() == rest.len() {
        return None;
    }
    let connector = after_digits.strip_prefix('-')?;
    if connector.is_empty() {
        None
    } else {
        Some(connector)
    }
}

#[cfg(test)]
mod tests {
    use super::strip_card_prefix;

    #[test]
    fn connector_nodes_only() {
        assert_eq!(strip_card_prefix("card0-eDP-1"), Some("eDP-1"));
        assert_eq!(strip_card_prefix("card1-HDMI-A-1"), Some("HDMI-A-1"));
        assert_eq!(strip_card_prefix("card0"), None);
        assert_eq!(strip_card_prefix("card0-"), None);
        assert_eq!(strip_card_prefix("renderD128"), None);
        assert_eq!(strip_card_prefix("version"), None);
    }
}
