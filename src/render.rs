//! Layout: logo block on the left, `key  value` info lines on the right.

use crate::color::Palette;

/// A single info line: either a raw pre-rendered string (title, separators,
/// color blocks) or a key/value pair to be aligned into columns.
pub enum Line {
    Raw(String),
    Kv(String, String),
}

/// Visible width of a string, ignoring ANSI SGR escape sequences. Assumes each
/// remaining `char` occupies one cell (true for our ASCII/box-drawing content).
pub fn visible_width(s: &str) -> usize {
    let mut w = 0usize;
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            // Skip a CSI sequence up to and including its final byte ('m' here).
            for n in it.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            w += 1;
        }
    }
    w
}

/// Return `s` with ANSI CSI (SGR) escape sequences removed.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            for n in it.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Render the whole output to stdout.
///
/// `term_width` of 0 disables value truncation (e.g. when piping).
pub fn render(logo: &[String], info: &[Line], pal: &Palette, term_width: usize) {
    let logo_w = logo.iter().map(|l| visible_width(l)).max().unwrap_or(0);
    let key_w = info
        .iter()
        .filter_map(|l| match l {
            Line::Kv(k, _) => Some(k.chars().count()),
            _ => None,
        })
        .max()
        .unwrap_or(0);

    let gap = if logo_w > 0 { 3 } else { 0 };
    const SEP: &str = "  "; // between key column and value

    let rows = logo.len().max(info.len());
    let mut out = String::new();
    for i in 0..rows {
        let logo_line = logo.get(i).map(String::as_str).unwrap_or("");
        let logo_pad = logo_w.saturating_sub(visible_width(logo_line));

        let info_str = match info.get(i) {
            Some(Line::Raw(s)) => s.clone(),
            Some(Line::Kv(k, v)) => {
                let mut value = v.clone();
                if term_width > 0 {
                    let used = logo_w + gap + key_w + SEP.len();
                    if term_width > used {
                        let avail = term_width - used;
                        if value.chars().count() > avail && avail > 1 {
                            let cut: String = value.chars().take(avail - 1).collect();
                            value = format!("{cut}…");
                        }
                    }
                }
                let key = pal.paint(pal.key, &format!("{:<width$}", k, width = key_w));
                format!("{key}{SEP}{value}")
            }
            None => String::new(),
        };

        let mut row = String::new();
        row.push_str(logo_line);
        row.push_str(&" ".repeat(logo_pad + gap));
        row.push_str(&info_str);
        // Trim the alignment padding that would otherwise trail logo-only rows.
        out.push_str(row.trim_end());
        out.push('\n');
    }
    print!("{out}");
}
