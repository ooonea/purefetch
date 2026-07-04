//! Small helpers shared by the detection modules.
#![allow(dead_code)]

use std::process::Command;

/// Read a file and trim trailing whitespace/newline. `None` if missing or empty.
pub fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim_end_matches(['\n', '\r', ' ', '\t']).to_string())
        .filter(|s| !s.is_empty())
}

/// First non-empty, trimmed line of a file.
pub fn first_line(path: &str) -> Option<String> {
    let s = std::fs::read_to_string(path).ok()?;
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// Run a command and capture its trimmed stdout. `None` on spawn failure,
/// non-zero exit, or empty output. Used only where no file-based source exists
/// (e.g. `gnome-shell --version`).
pub fn cmd(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Run a shell command line via `sh -c` and return its first non-empty output
/// line. Powers the `--exec` custom modules.
pub fn sh(command: &str) -> Option<String> {
    let out = Command::new("sh").arg("-c").arg(command).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// Run a shell command line via `sh -c` and return its full stdout verbatim.
/// Used to generate a logo dynamically (`--logo-exec`).
pub fn sh_raw(command: &str) -> Option<String> {
    let out = Command::new("sh").arg("-c").arg(command).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Format a byte count with IEC units, e.g. `62.61 GiB`.
pub fn human_iec(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", UNITS[i])
    }
}

/// Rounded integer percentage of `part` over `total`.
pub fn percent(part: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        ((part as f64 / total as f64) * 100.0).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::{human_iec, percent};

    #[test]
    fn iec_units_and_rounding() {
        assert_eq!(human_iec(0), "0 B");
        assert_eq!(human_iec(512), "512 B");
        assert_eq!(human_iec(1024), "1.00 KiB");
        assert_eq!(human_iec(1536), "1.50 KiB");
        assert_eq!(human_iec(1024 * 1024), "1.00 MiB");
        assert_eq!(human_iec(3u64 * 1024 * 1024 * 1024), "3.00 GiB");
    }

    #[test]
    fn percent_rounds_and_guards_zero() {
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(1, 0), 0);
        assert_eq!(percent(1, 2), 50);
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(2, 3), 67);
        assert_eq!(percent(3, 3), 100);
    }
}
