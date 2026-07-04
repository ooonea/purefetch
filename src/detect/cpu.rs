//! CPU: cleaned model name @ max frequency,
//! e.g. "Intel(R) Core(TM) i7-9850H @ 4.60 GHz".
//!
//! Architectures label the CPU differently in /proc/cpuinfo: x86 and most ARM
//! use "model name"; riscv uses "uarch" or the "isa" string; ppc uses "cpu";
//! some ARM SoCs use "Hardware". We take the first present, in that order.
use crate::detect::{Row, Rows};

pub fn detect() -> Rows {
    let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") else {
        return Vec::new();
    };
    let Some(model) = model_name(&cpuinfo) else {
        return Vec::new();
    };

    let name = clean_model(&model);
    let value = match max_freq_ghz() {
        Some(f) => format!("{name} @ {f:.2} GHz"),
        None => name,
    };
    vec![Row::val(value)]
}

/// The CPU name from /proc/cpuinfo, trying each key in preference order.
fn model_name(cpuinfo: &str) -> Option<String> {
    const KEYS: [&str; 6] = ["model name", "Hardware", "cpu model", "cpu", "uarch", "isa"];
    for key in KEYS {
        for line in cpuinfo.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim() == key {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Drop the trailing " CPU @ x.xxGHz" but keep the vendor "(R)"/"(TM)" marks,
/// matching fastfetch's `{name}` field. A no-op for riscv/ppc name strings.
fn clean_model(s: &str) -> String {
    let base = s.split(" @ ").next().unwrap_or(s).trim();
    let base = base.strip_suffix(" CPU").unwrap_or(base);
    base.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn max_freq_ghz() -> Option<f64> {
    crate::util::read_trim("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|khz| khz / 1_000_000.0)
}
