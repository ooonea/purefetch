//! Disk: used / total (percent). On a normal filesystem this is statfs(2) on
//! `/`. On a ZFS root, statfs only sees the root DATASET (a few GiB of the
//! pool), so we report the whole pool instead — as `zpool list` does — labelled
//! by the pool name. On an impermanence/tmpfs root (`/` is tmpfs) we locate the
//! pool via `/nix` instead, so the disk still reads as the real pool, not the
//! few-MiB RAM root.
use crate::detect::{Row, Rows};
#[cfg(target_os = "linux")]
use crate::util::cmd;
use crate::util::{human_iec, percent};

pub fn detect() -> Rows {
    #[cfg(target_os = "linux")]
    if let Some(row) = zfs_pool_row() {
        return vec![row];
    }
    let Some(d) = crate::sys::disk_usage("/") else {
        return Vec::new();
    };
    vec![Row::val(format!(
        "{} / {} ({}%)",
        human_iec(d.used),
        human_iec(d.total),
        percent(d.used, d.total)
    ))]
}

/// When `/` is ZFS, report its pool's allocated/size via `zpool list`.
#[cfg(target_os = "linux")]
fn zfs_pool_row() -> Option<Row> {
    let pool = zfs_root_pool()?;
    // `zpool list -Hp -o size,alloc <pool>` -> "<size>\t<alloc>" in bytes.
    let out = cmd("zpool", &["list", "-Hp", "-o", "size,alloc", &pool])?;
    let mut f = out.split_whitespace();
    let size: u64 = f.next()?.parse().ok()?;
    let alloc: u64 = f.next()?.parse().ok()?;
    if size == 0 {
        return None;
    }
    Some(Row::keyed(
        format!("Disk ({pool})"),
        format!(
            "{} / {} ({}%)",
            human_iec(alloc),
            human_iec(size),
            percent(alloc, size)
        ),
    ))
}

/// The ZFS pool the system lives on: `/`'s pool on a ZFS-root box, or — when `/`
/// is tmpfs (impermanence) — the pool backing `/nix`, where the store actually
/// lives. Returns the first component of the dataset name (`zroot/nix` -> `zroot`).
#[cfg(target_os = "linux")]
fn zfs_root_pool() -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    let pool_at = |want: &str| {
        mounts.lines().find_map(|line| {
            let mut f = line.split_whitespace();
            let dev = f.next()?;
            let mnt = f.next()?;
            let fstype = f.next()?;
            (mnt == want && fstype == "zfs")
                .then(|| dev.split('/').next().unwrap_or(dev).to_string())
        })
    };
    // Prefer the root's own pool; on a tmpfs/impermanence root fall back to /nix.
    pool_at("/").or_else(|| pool_at("/nix"))
}
