//! macOS platform layer — `extern "C"` bindings to libSystem, no libc crate.
//!
//! Raw syscalls are not a stable ABI on macOS: the only supported kernel
//! interface is libSystem, which Rust's std already links unconditionally on
//! darwin targets, so declaring the handful of C functions we need directly
//! keeps the crate at zero external dependencies here too. Struct layouts and
//! constants mirror Apple's SDK headers (xnu: bsd/sys/mount.h,
//! bsd/sys/sysctl.h, mach/vm_statistics.h, libproc.h) and agree with the rust
//! `libc` crate's darwin definitions.
#![allow(dead_code)]

use super::DiskUsage;
use std::ffi::c_void;
use std::os::raw::c_ulong;

extern "C" {
    fn ioctl(fd: i32, request: c_ulong, ...) -> i32;
    fn isatty(fd: i32) -> i32;
    fn gethostname(name: *mut u8, len: usize) -> i32;
    fn sysctlbyname(
        name: *const u8,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> i32;
    fn mach_host_self() -> u32;
    fn host_statistics64(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut c_void, buffersize: i32) -> i32;
    // Everywhere but arm64 the plain symbol is the legacy 32-bit-inode
    // variant; the $INODE64 suffix selects the modern layout (same cfg rule
    // as the libc crate). arm64 only ever had the 64-bit one.
    #[cfg_attr(not(target_arch = "aarch64"), link_name = "statfs$INODE64")]
    fn statfs(path: *const u8, buf: *mut Statfs) -> i32;
}

// struct statfs, 64-bit-inode variant (xnu __DARWIN_STRUCT_STATFS64) — 2168
// bytes, identical layout on arm64 and x86_64.
#[repr(C)]
struct Statfs {
    f_bsize: u32,
    f_iosize: i32,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_fsid: [i32; 2],
    f_owner: u32,
    f_type: u32,
    f_flags: u32,
    f_fssubtype: u32,
    f_fstypename: [u8; 16],
    f_mntonname: [u8; 1024],
    f_mntfromname: [u8; 1024],
    f_flags_ext: u32,
    f_reserved: [u32; 7],
}

/// Disk usage of the filesystem containing `path`, via statfs(2). On APFS
/// every volume shares the container's space, so used = total - available is
/// the honest whole-disk figure (and what fastfetch reports); total - free
/// would hide purgeable/snapshot space.
pub fn disk_usage(path: &str) -> Option<DiskUsage> {
    let mut cpath = Vec::with_capacity(path.len() + 1);
    cpath.extend_from_slice(path.as_bytes());
    cpath.push(0); // NUL terminate
    let mut sb: Statfs = unsafe { std::mem::zeroed() };
    if unsafe { statfs(cpath.as_ptr(), &mut sb) } != 0 {
        return None;
    }
    let bs = sb.f_bsize as u64;
    let total = sb.f_blocks.saturating_mul(bs);
    if total == 0 {
        return None;
    }
    let avail = sb.f_bavail.saturating_mul(bs);
    Some(DiskUsage {
        total,
        used: total.saturating_sub(avail),
        avail,
    })
}

#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

// _IOR('t', 104, struct winsize): darwin encodes direction and size into the
// request, so the value differs from Linux's.
const TIOCGWINSZ: c_ulong = 0x4008_7468;

/// Terminal width in columns (stdout), or 80 if it cannot be determined.
pub fn term_width() -> usize {
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let r = unsafe { ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
    if r == 0 && ws.ws_col > 0 {
        ws.ws_col as usize
    } else {
        80
    }
}

/// Whether stdout is a terminal (used to decide on color output).
pub fn stdout_is_tty() -> bool {
    unsafe { isatty(1) == 1 }
}

/// The node name, for the `user@host` title.
pub fn hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    if unsafe { gethostname(buf.as_mut_ptr(), buf.len()) } != 0 {
        return None;
    }
    buf_str(&buf)
}

// proc_pidinfo(PROC_PIDTBSDINFO) fills this stable libproc struct — it avoids
// the pointer-laden kinfo_proc layout entirely.
const PROC_PIDTBSDINFO: i32 = 3;

#[repr(C)]
struct ProcBsdinfo {
    pbi_flags: u32,
    pbi_status: u32,
    pbi_xstatus: u32,
    pbi_pid: u32,
    pbi_ppid: u32,
    pbi_uid: u32,
    pbi_gid: u32,
    pbi_ruid: u32,
    pbi_rgid: u32,
    pbi_svuid: u32,
    pbi_svgid: u32,
    rfu_1: u32,
    pbi_comm: [u8; 16],
    pbi_name: [u8; 32],
    pbi_nfiles: u32,
    pbi_pgid: u32,
    pbi_pjobc: u32,
    e_tdev: u32,
    e_tpgid: u32,
    pbi_nice: i32,
    pbi_start_tvsec: u64,
    pbi_start_tvusec: u64,
}

/// Parent pid and short command name of `pid`, for the parent-chain walks in
/// `detect::{shell,terminal}`. `pbi_name` (32 bytes) beats `pbi_comm`'s
/// 16-byte truncation; the name is empty when neither is readable (the walk
/// just skips it and keeps climbing).
pub fn ppid_comm(pid: u32) -> Option<(u32, String)> {
    let mut info: ProcBsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<ProcBsdinfo>() as i32;
    let r = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDTBSDINFO,
            0,
            &mut info as *mut ProcBsdinfo as *mut c_void,
            size,
        )
    };
    if r <= 0 {
        return None;
    }
    let comm = buf_str(&info.pbi_name)
        .or_else(|| buf_str(&info.pbi_comm))
        .unwrap_or_default();
    Some((info.pbi_ppid, comm))
}

/// A NUL-terminated copy of `name` for sysctlbyname.
fn cname(name: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(name.len() + 1);
    v.extend_from_slice(name.as_bytes());
    v.push(0);
    v
}

/// String from a NUL-terminated (or full) C byte buffer; None if empty.
fn buf_str(buf: &[u8]) -> Option<String> {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let s = String::from_utf8_lossy(&buf[..end]).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// String sysctl by name (the usual two-call size protocol).
pub fn sysctl_string(name: &str) -> Option<String> {
    let n = cname(name);
    let mut len = 0usize;
    let probe = unsafe {
        sysctlbyname(
            n.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if probe != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    let r = unsafe {
        sysctlbyname(
            n.as_ptr(),
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        return None;
    }
    buf.truncate(len);
    buf_str(&buf)
}

/// Integer sysctl by name. Kernel integers come in 4- or 8-byte flavors; both
/// darwin arches are little-endian, so reading either into a zeroed u64 is
/// exact (a 4-byte value fills the low half).
pub fn sysctl_u64(name: &str) -> Option<u64> {
    let n = cname(name);
    let mut val: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let r = unsafe {
        sysctlbyname(
            n.as_ptr(),
            &mut val as *mut u64 as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        None
    } else {
        Some(val)
    }
}

// struct timeval on 64-bit darwin: time_t = i64, suseconds_t = i32; repr(C)
// pads it to 16 bytes, which matches the kernel's copy-out size.
#[repr(C)]
struct Timeval {
    tv_sec: i64,
    tv_usec: i32,
}

/// Boot time in seconds since the epoch (kern.boottime).
pub fn boottime_secs() -> Option<u64> {
    let n = cname("kern.boottime");
    let mut tv = Timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut len = std::mem::size_of::<Timeval>();
    let r = unsafe {
        sysctlbyname(
            n.as_ptr(),
            &mut tv as *mut Timeval as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 || tv.tv_sec <= 0 {
        None
    } else {
        Some(tv.tv_sec as u64)
    }
}

// vm.swapusage payload (xnu struct xsw_usage), 32 bytes, no packing.
#[repr(C)]
struct XswUsage {
    xsu_total: u64,
    xsu_avail: u64,
    xsu_used: u64,
    xsu_pagesize: u32,
    xsu_encrypted: i32,
}

/// Swap (total, used) in bytes; total is 0 when no swap files exist.
pub fn swap_usage() -> Option<(u64, u64)> {
    let n = cname("vm.swapusage");
    let mut xsw: XswUsage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<XswUsage>();
    let r = unsafe {
        sysctlbyname(
            n.as_ptr(),
            &mut xsw as *mut XswUsage as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        None
    } else {
        Some((xsw.xsu_total, xsw.xsu_used))
    }
}

const HOST_VM_INFO64: i32 = 4;

// mach vm_statistics64 (xnu mach/vm_statistics.h), full current layout. Only
// the leading fields are consumed; the tail keeps the buffer big enough that
// any kernel revision fills at most what we pass. The u32 fields come in even
// runs, so every u64 already sits on an 8-byte offset and plain repr(C)
// reproduces xnu's natural (aligned(8)) layout exactly.
#[repr(C)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
    swapped_count: u64,
    total_tag_storage_pages: u64,
    nontag_pageable_tag_storage_pages: u64,
    nontag_wired_tag_storage_pages: u64,
    free_tag_storage_pages: u64,
    tag_storing_tag_storage_pages: u64,
    total_tagged_pages: u64,
    resident_tagged_pages: u64,
    compressed_tagged_pages: u64,
    tagged_compressions: u64,
    tagged_decompressions: u64,
    compressed_tag_storage_bytes: u64,
    speculative_pages_created: u64,
    speculative_pages_activated: u64,
    swap_count: u64,
    empty_tag_storing_tag_storage_pages: u64,
    executable_count: u64,
    shared_region_count: u64,
    boot_stolen_count: u64,
    secluded_count: u64,
    active_internal_count: u64,
    inactive_internal_count: u64,
    active_external_count: u64,
    inactive_external_count: u64,
    purgeable_pageable_count: u64,
    purgeable_wired_count: u64,
    background_internal_count: u64,
    background_external_count: u64,
    donated_count: u64,
    realtime_count: u64,
    max_mem_count: u64,
    phantom_ghosts_found: u64,
    phantom_ghosts_added: u64,
}

/// Memory (used, total) in bytes. Used follows vm_stat / Activity Monitor:
/// total minus really-free pages (free - speculative) and the file cache
/// (external), both of which the kernel reclaims freely.
pub fn memory_used_total() -> Option<(u64, u64)> {
    let total = sysctl_u64("hw.memsize").filter(|t| *t > 0)?;
    let page = sysctl_u64("hw.pagesize").filter(|p| *p > 0)?;
    let mut vs: VmStatistics64 = unsafe { std::mem::zeroed() };
    let mut count = (std::mem::size_of::<VmStatistics64>() / 4) as u32;
    let kr = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            &mut vs as *mut VmStatistics64 as *mut i32,
            &mut count,
        )
    };
    if kr != 0 {
        return None;
    }
    let reclaimable = (vs.free_count.saturating_sub(vs.speculative_count) as u64)
        .saturating_add(vs.external_page_count as u64)
        .saturating_mul(page);
    Some((total.saturating_sub(reclaimable), total))
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGGetActiveDisplayList(max: u32, ids: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayCopyDisplayMode(display: u32) -> *mut c_void;
    fn CGDisplayModeGetPixelWidth(mode: *mut c_void) -> usize;
    fn CGDisplayModeGetPixelHeight(mode: *mut c_void) -> usize;
    fn CGDisplayModeRelease(mode: *mut c_void);
}

/// Native pixel size of each active display. A headless/ssh session gets a
/// success with zero displays, so this is safe to call unconditionally.
pub fn displays() -> Vec<(usize, usize)> {
    const MAX: usize = 16;
    let mut ids = [0u32; MAX];
    let mut n: u32 = 0;
    if unsafe { CGGetActiveDisplayList(MAX as u32, ids.as_mut_ptr(), &mut n) } != 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for &id in ids.iter().take(n as usize) {
        let mode = unsafe { CGDisplayCopyDisplayMode(id) };
        if mode.is_null() {
            continue;
        }
        let w = unsafe { CGDisplayModeGetPixelWidth(mode) };
        let h = unsafe { CGDisplayModeGetPixelHeight(mode) };
        unsafe { CGDisplayModeRelease(mode) };
        if w > 0 && h > 0 {
            out.push((w, h));
        }
    }
    out
}
