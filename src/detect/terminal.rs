//! Terminal: terminal emulator (+ version if cheap), e.g. "kitty 0.41.1".
//! Primary source: walk the parent-process chain and pick the first ancestor
//! that is a known terminal (shells and purefetch itself are simply skipped).
//! Fallback: environment hints ($TERM, $KITTY_WINDOW_ID, $TERM_PROGRAM, ...),
//! needed when we run under `bash -c`/`script` and the chain has no terminal.
use crate::detect::{Row, Rows};

pub fn detect() -> Rows {
    let term = match walk_parents().or_else(from_env) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let value = match version_of(term) {
        Some(v) => format!("{term} {v}"),
        None => term.to_string(),
    };
    vec![Row::val(value)]
}

/// Climb the parent chain from our own ppid; return the first known terminal.
fn walk_parents() -> Option<&'static str> {
    let mut pid = ppid_of("/proc/self/stat")?;
    // Guard against cycles / runaway with a hard cap; stop at init.
    for _ in 0..64 {
        if pid <= 1 {
            break;
        }
        if let Some(comm) = crate::util::read_trim(&format!("/proc/{pid}/comm")) {
            if let Some(canon) = canonical(&comm) {
                return Some(canon);
            }
        }
        match ppid_of(&format!("/proc/{pid}/stat")) {
            Some(next) if next != pid => pid = next,
            _ => break,
        }
    }
    None
}

/// Parse the ppid from a `/proc/<pid>/stat` file. The comm field is enclosed in
/// parentheses and may itself contain spaces or ')', so we split on the LAST
/// ')': the remainder is "state ppid ...", where ppid is the 2nd whitespace field.
fn ppid_of(stat_path: &str) -> Option<i64> {
    let text = crate::util::read_trim(stat_path)?;
    let rest = &text[text.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse::<i64>().ok()
}

/// Map a process `comm` (possibly truncated to 15 chars by the kernel) to a
/// canonical terminal display name, or None if it is not a known terminal.
fn canonical(comm: &str) -> Option<&'static str> {
    // (comm-as-seen, display-name). Ordered longest-first is not required since
    // we match exactly, with a prefix rule only for the 15-char truncation case.
    const TERMS: &[(&str, &str)] = &[
        ("kitty", "kitty"),
        ("alacritty", "alacritty"),
        ("foot", "foot"),
        ("wezterm-gui", "wezterm"),
        ("wezterm", "wezterm"),
        ("konsole", "konsole"),
        ("gnome-terminal-server", "gnome-terminal"),
        ("xterm", "xterm"),
        ("st", "st"),
        ("urxvt", "urxvt"),
        ("rxvt", "rxvt"),
        ("terminator", "terminator"),
        ("tilix", "tilix"),
        ("contour", "contour"),
        ("ghostty", "ghostty"),
        ("tmux", "tmux"),
        ("screen", "screen"),
    ];
    for (name, display) in TERMS {
        if comm == *name {
            return Some(display);
        }
        // Kernel truncates comm to 15 chars; match a long name by its prefix.
        if comm.len() == 15 && name.starts_with(comm) {
            return Some(display);
        }
    }
    None
}

/// Resolve the terminal from environment hints when the chain gave nothing.
fn from_env() -> Option<&'static str> {
    let has = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if has("KITTY_WINDOW_ID") {
        return Some("kitty");
    }
    if has("ALACRITTY_SOCKET") {
        return Some("alacritty");
    }
    if has("WEZTERM_EXECUTABLE") {
        return Some("wezterm");
    }
    if has("KONSOLE_VERSION") {
        return Some("konsole");
    }
    if has("TERMINATOR_UUID") {
        return Some("terminator");
    }
    if let Ok(tp) = std::env::var("TERM_PROGRAM") {
        if let Some(t) = from_token(&tp) {
            return Some(t);
        }
    }
    if let Ok(term) = std::env::var("TERM") {
        if let Some(t) = from_term(&term) {
            return Some(t);
        }
    }
    // $VTE_VERSION is set by every VTE-based terminal; gnome-terminal is the
    // most likely and matches the label fastfetch shows. Checked last.
    if has("VTE_VERSION") {
        return Some("gnome-terminal");
    }
    None
}

/// Map a $TERM value (e.g. "xterm-kitty", "foot-extra") to a terminal.
fn from_term(term: &str) -> Option<&'static str> {
    let t = term.to_ascii_lowercase();
    if t.contains("kitty") {
        Some("kitty")
    } else if t.contains("alacritty") {
        Some("alacritty")
    } else if t.contains("ghostty") {
        Some("ghostty")
    } else if t.contains("foot") {
        Some("foot")
    } else if t.contains("wezterm") {
        Some("wezterm")
    } else if t.contains("contour") {
        Some("contour")
    } else if t.starts_with("rxvt-unicode") {
        Some("urxvt")
    } else if t.starts_with("rxvt") {
        Some("rxvt")
    } else if t.starts_with("st-") || t == "st" {
        Some("st")
    } else if t.starts_with("tmux") {
        Some("tmux")
    } else if t.starts_with("screen") {
        Some("screen")
    } else {
        None
    }
}

/// Map a $TERM_PROGRAM token to a terminal.
fn from_token(tp: &str) -> Option<&'static str> {
    let t = tp.to_ascii_lowercase();
    match t.as_str() {
        "kitty" => Some("kitty"),
        "alacritty" => Some("alacritty"),
        "ghostty" => Some("ghostty"),
        "wezterm" => Some("wezterm"),
        "tmux" => Some("tmux"),
        "konsole" => Some("konsole"),
        _ if t.contains("foot") => Some("foot"),
        _ => None,
    }
}

/// Best-effort version for a small allowlist of terminals that answer
/// `--version` cheaply and safely. Returns the first version-like token.
fn version_of(term: &str) -> Option<String> {
    // Only probe binaries known to support --version without side effects.
    const SAFE: &[&str] = &[
        "kitty",
        "alacritty",
        "foot",
        "wezterm",
        "konsole",
        "ghostty",
        "contour",
        "tilix",
        "terminator",
        "gnome-terminal",
        "xterm",
        "tmux",
        "screen",
    ];
    if !SAFE.contains(&term) {
        return None;
    }
    let out = crate::util::cmd(term, &["--version"])?;
    out.lines().next().and_then(version_token)
}

/// Pick the first whitespace token that looks like a version number,
/// e.g. "kitty 0.41.1 created by ..." -> "0.41.1". A leading 'v' is stripped.
fn version_token(line: &str) -> Option<String> {
    for tok in line.split_whitespace() {
        let candidate = tok
            .strip_prefix('v')
            .or_else(|| tok.strip_prefix('V'))
            .unwrap_or(tok);
        if candidate.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(candidate.to_string());
        }
    }
    None
}
