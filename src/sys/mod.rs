//! Platform layer — everything OS-specific lives in one per-OS submodule.
//!
//! The rest of the crate only sees this module's re-exported API:
//! * `disk_usage(path)` — statfs on the filesystem containing `path`
//! * `term_width()` — terminal width in columns (80 fallback)
//! * `stdout_is_tty()` — color/width gate
//! * `hostname()` — the node name for the `user@host` title
//! * `ppid_comm(pid)` — (parent pid, short command name), for the parent-chain
//!   walks in `detect::{shell,terminal}`
//! * `kill_process_group(pid)` — stop a command and its descendants
//!
//! Linux issues raw syscalls and reads `/proc` (no libc, no external crates);
//! macOS binds `extern "C"` to libSystem — the only stable syscall ABI there —
//! which std links unconditionally, so the crate stays free of external
//! dependencies on both.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::*;

// `avail` is unused today but is part of the statfs contract both platform
// layers fill in; keep it rather than lose the information.
#[allow(dead_code)]
pub struct DiskUsage {
    pub total: u64,
    pub used: u64,
    pub avail: u64,
}
