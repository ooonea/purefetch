//! Minimal syscall shims — no libc, no external crates.
//!
//! The whole point of purefetch is to be written *entirely* in Rust, so instead
//! of binding to C's `statvfs`/`ioctl` we issue the raw Linux syscalls directly
//! on x86_64, aarch64, riscv64 and loongarch64. Everything else in the crate
//! uses `/proc`, `/sys`, std.
//!
//! On other architectures these fall back to conservative defaults so the crate
//! still builds.
#![allow(dead_code)]

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    core::arch::asm!(
        "syscall",
        inlateout("rax") n as isize => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        out("rcx") _,
        out("r11") _,
        options(nostack),
    );
    ret
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    // aarch64 Linux ABI: nr in x8, args in x0..x2, return in x0; the kernel
    // preserves all other registers, so no extra clobbers are needed.
    core::arch::asm!(
        "svc #0",
        in("x8") n,
        inlateout("x0") a1 => ret,
        in("x1") a2,
        in("x2") a3,
        options(nostack),
    );
    ret
}

#[cfg(target_arch = "riscv64")]
#[inline]
unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    // riscv64 Linux ABI: nr in a7, args in a0..a2, return in a0.
    core::arch::asm!(
        "ecall",
        in("a7") n,
        inlateout("a0") a1 => ret,
        in("a1") a2,
        in("a2") a3,
        options(nostack),
    );
    ret
}

#[cfg(target_arch = "loongarch64")]
#[inline]
unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    // loongarch64 Linux ABI: nr in a7 ($r11), args in a0..a2 ($r4..$r6), return in a0.
    core::arch::asm!(
        "syscall 0",
        in("$r11") n,
        inlateout("$r4") a1 => ret,
        in("$r5") a2,
        in("$r6") a3,
        options(nostack),
    );
    ret
}

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
)))]
unsafe fn syscall3(_n: usize, _a1: usize, _a2: usize, _a3: usize) -> isize {
    -1
}

// Syscall numbers are per-architecture: aarch64, riscv64 and loongarch64 share
// the "asm-generic" table (statfs=43, ioctl=29); x86_64 has its own. The ioctl
// request codes (TIOCGWINSZ/TCGETS) are the asm-generic values on all of these.
#[cfg(target_arch = "x86_64")]
const SYS_IOCTL: usize = 16;
#[cfg(target_arch = "x86_64")]
const SYS_STATFS: usize = 137;
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
))]
const SYS_IOCTL: usize = 29;
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
))]
const SYS_STATFS: usize = 43;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
)))]
const SYS_IOCTL: usize = 0;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
)))]
const SYS_STATFS: usize = 0;

const TIOCGWINSZ: usize = 0x5413;
const TCGETS: usize = 0x5401;

// struct statfs on x86_64 Linux (see `man 2 statfs`). 120 bytes.
#[repr(C)]
struct Statfs {
    f_type: i64,
    f_bsize: i64,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_fsid: [i32; 2],
    f_namelen: i64,
    f_frsize: i64,
    f_flags: i64,
    f_spare: [i64; 4],
}

pub struct DiskUsage {
    pub total: u64,
    pub used: u64,
    pub avail: u64,
}

/// Disk usage of the filesystem containing `path`, via the `statfs(2)` syscall.
pub fn disk_usage(path: &str) -> Option<DiskUsage> {
    let mut cpath = Vec::with_capacity(path.len() + 1);
    cpath.extend_from_slice(path.as_bytes());
    cpath.push(0); // NUL terminate

    let mut sb: Statfs = unsafe { core::mem::zeroed() };
    let r = unsafe {
        syscall3(
            SYS_STATFS,
            cpath.as_ptr() as usize,
            &mut sb as *mut Statfs as usize,
            0,
        )
    };
    if r < 0 || sb.f_bsize <= 0 {
        return None;
    }
    let bs = sb.f_bsize as u64;
    let total = sb.f_blocks.saturating_mul(bs);
    if total == 0 {
        return None;
    }
    let avail = sb.f_bavail.saturating_mul(bs);
    let used = total.saturating_sub(sb.f_bfree.saturating_mul(bs));
    Some(DiskUsage { total, used, avail })
}

#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

/// Terminal width in columns (stdout), or 80 if it cannot be determined.
pub fn term_width() -> usize {
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let r = unsafe {
        syscall3(
            SYS_IOCTL,
            1, // stdout
            TIOCGWINSZ,
            &mut ws as *mut Winsize as usize,
        )
    };
    if r >= 0 && ws.ws_col > 0 {
        ws.ws_col as usize
    } else {
        80
    }
}

/// Whether stdout is a terminal (used to decide on color output).
pub fn stdout_is_tty() -> bool {
    // TCGETS returns 0 on a tty, -ENOTTY otherwise. A 64-byte buffer safely
    // holds the kernel `struct termios`.
    let mut buf = [0u8; 64];
    unsafe { syscall3(SYS_IOCTL, 1, TCGETS, buf.as_mut_ptr() as usize) >= 0 }
}
