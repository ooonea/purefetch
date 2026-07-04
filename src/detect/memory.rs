//! Memory: used / total (percent), e.g. "37.69 GiB / 62.61 GiB (60%)".
//! Used = MemTotal - MemAvailable, matching `free` and htop. On ZFS the ARC
//! counts as used because MemAvailable does not treat it as reclaimable — which
//! is honest: that RAM is genuinely occupied by the cache until the kernel
//! reclaims it under pressure.
use crate::detect::{Row, Rows};
use crate::util::{human_iec, percent};

pub fn detect() -> Rows {
    let Ok(info) = std::fs::read_to_string("/proc/meminfo") else {
        return Vec::new();
    };
    let mut total = 0u64;
    let mut available = 0u64;
    for line in info.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = parse_kb(v);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            available = parse_kb(v);
        }
    }
    if total == 0 {
        return Vec::new();
    }

    let used = total.saturating_sub(available);
    vec![Row::val(format!(
        "{} / {} ({}%)",
        human_iec(used),
        human_iec(total),
        percent(used, total)
    ))]
}

fn parse_kb(s: &str) -> u64 {
    s.split_whitespace()
        .next()
        .and_then(|n| n.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::parse_kb;

    #[test]
    fn parse_kb_to_bytes() {
        assert_eq!(parse_kb("  62518234 kB"), 62_518_234 * 1024);
        assert_eq!(parse_kb("0 kB"), 0);
        assert_eq!(parse_kb("garbage"), 0);
        assert_eq!(parse_kb(""), 0);
    }
}
