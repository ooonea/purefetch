//! GPU: graphics adapter model, e.g. "Quadro RTX 3000".
//! Primary source: /proc/driver/nvidia/gpus/*/information ("Model:" line).
//! Fallback: lspci, for non-NVIDIA adapters or when the nvidia proc is absent.
use crate::detect::{Row, Rows};

pub fn detect() -> Rows {
    let mut rows = nvidia_gpus();
    if rows.is_empty() {
        rows = lspci_gpus();
    }
    rows
}

/// One Row per NVIDIA GPU, parsed from the driver's proc information file.
fn nvidia_gpus() -> Rows {
    let mut rows = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc/driver/nvidia/gpus") else {
        return rows;
    };
    for entry in entries.flatten() {
        let info = entry.path().join("information");
        let Some(path) = info.to_str() else { continue };
        let Some(text) = crate::util::read_trim(path) else {
            continue;
        };
        for line in text.lines() {
            // The "Model:" value is tab-separated after the colon.
            if let Some((k, v)) = line.split_once(':') {
                if k.trim() == "Model" {
                    let model = v.trim();
                    if !model.is_empty() {
                        rows.push(Row::val(model));
                    }
                    break;
                }
            }
        }
    }
    rows
}

/// Fallback: parse `lspci` for any graphics controller.
fn lspci_gpus() -> Rows {
    let Some(out) = crate::util::cmd("lspci", &[]) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for line in out.lines() {
        if !(line.contains("VGA compatible controller")
            || line.contains("3D controller")
            || line.contains("Display controller"))
        {
            continue;
        }
        // Text after the last ": " is the vendor + device description.
        let Some((_, desc)) = line.rsplit_once(": ") else {
            continue;
        };
        let name = clean_lspci(desc.trim());
        if !name.is_empty() {
            rows.push(Row::val(name));
        }
    }
    rows
}

/// Strip a known vendor prefix and prefer a bracketed marketing name.
fn clean_lspci(desc: &str) -> String {
    let stripped = desc
        .strip_prefix("NVIDIA Corporation ")
        .or_else(|| desc.strip_prefix("Intel Corporation "))
        .or_else(|| desc.strip_prefix("Advanced Micro Devices, Inc. "))
        .unwrap_or(desc);
    // AMD prefixes the die with a bracketed vendor tag ("[AMD/ATI] Navi 31
    // [Radeon ...]"); drop it so it isn't mistaken for the marketing name.
    let stripped = stripped
        .strip_prefix("[AMD/ATI] ")
        .or_else(|| stripped.strip_prefix("[AMD] "))
        .unwrap_or(stripped);
    // Drop a trailing PCI revision, e.g. "Raphael (rev c9)" -> "Raphael".
    let stripped = stripped
        .rsplit_once(" (rev ")
        .map_or(stripped, |(head, _)| head);
    // Prefer the marketing name in "[ ... ]" if present.
    if let Some(start) = stripped.find('[') {
        if let Some(end) = stripped[start + 1..].find(']') {
            let inner = stripped[start + 1..start + 1 + end].trim();
            if !inner.is_empty() {
                return inner.to_string();
            }
        }
    }
    stripped.trim().to_string()
}
