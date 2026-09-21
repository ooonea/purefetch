//! Small helpers shared by the detection modules.
#![allow(dead_code)]

use std::io::{ErrorKind, Read};
use std::os::fd::OwnedFd;
use std::os::unix::{net::UnixStream, process::CommandExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn command_output(mut command: Command) -> Option<Vec<u8>> {
    let (mut reader, writer) = UnixStream::pair().ok()?;
    reader.set_nonblocking(true).ok()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    drop(command);
    let start = Instant::now();
    let mut exited = false;
    let output = (|| {
        let mut output = Vec::new();
        let mut buf = [0; 8192];
        while start.elapsed() < Duration::from_secs(2) {
            match reader.read(&mut buf) {
                Ok(0) => {
                    if let Some(status) = child.try_wait().ok()? {
                        exited = true;
                        return status.success().then_some(output);
                    }
                }
                Ok(n) => {
                    output.extend_from_slice(&buf[..n]);
                    continue;
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(_) => return None,
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    })();
    if !exited {
        // Descendants can outlive the command and retain its stdout.
        crate::sys::kill_process_group(child.id());
        let _ = child.kill();
        let _ = child.wait();
    }
    output
}

/// Read a file and trim trailing whitespace/newline. `None` if missing or empty.
pub fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim_end().to_string())
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
    let mut command = Command::new(prog);
    command.args(args);
    let out = command_output(command)?;
    let s = String::from_utf8_lossy(&out).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Run a shell command line via `sh -c` and return its first non-empty output
/// line. Powers the `--exec` custom modules.
pub fn sh(command: &str) -> Option<String> {
    let out = sh_raw(command)?;
    out.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// Run a shell command line via `sh -c` and return its full stdout verbatim.
/// Used to generate a logo dynamically (`--logo-exec`).
pub fn sh_raw(command: &str) -> Option<String> {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    let out = command_output(shell)?;
    Some(String::from_utf8_lossy(&out).into_owned())
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

/// The raw value of the first `"key" = ...` property in an `ioreg` text dump.
/// The lines carry tree decoration (`| `), so the needle is searched anywhere;
/// the leading quote in it keeps longer key names from matching.
/// macOS-only data source (IORegistry); the parsers are OS-neutral for tests.
#[cfg(any(target_os = "macos", test))]
pub fn ioreg_find(dump: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\" = ");
    dump.lines().find_map(|l| {
        l.find(&needle)
            .map(|pos| l[pos + needle.len()..].to_string())
    })
}

/// Decode one ioreg property value into a string. ioreg prints CFString as
/// `"str"`, printable CFData as `<"str">`, and binary CFData as `<hexbytes>`
/// (often a NUL-terminated ASCII string, e.g. the `model` of a GPU).
#[cfg(any(target_os = "macos", test))]
pub fn ioreg_decode(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if let Some(inner) = raw
        .strip_prefix("<\"")
        .and_then(|r| r.strip_suffix("\">"))
        .or_else(|| raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')))
    {
        return non_empty(inner);
    }
    let hex = raw.strip_prefix('<').and_then(|r| r.strip_suffix('>'))?;
    let hex: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if hex.is_empty() || hex.len() % 2 != 0 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0))
        .collect();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    non_empty(&String::from_utf8_lossy(&bytes[..end]))
}

/// A string ioreg property, found and decoded.
#[cfg(any(target_os = "macos", test))]
pub fn ioreg_string(dump: &str, key: &str) -> Option<String> {
    ioreg_decode(&ioreg_find(dump, key)?)
}

/// An integer ioreg property (printed bare, e.g. `"gpu-core-count" = 8`).
#[cfg(any(target_os = "macos", test))]
pub fn ioreg_u64(dump: &str, key: &str) -> Option<u64> {
    ioreg_find(dump, key)?.trim().parse().ok()
}

#[cfg(any(target_os = "macos", test))]
fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{cmd, human_iec, ioreg_string, ioreg_u64, percent};

    #[test]
    fn command_output_checks_status_and_drains_large_stdout() {
        assert_eq!(
            cmd("sh", &["-c", "printf good; printf ignored >&2"]).as_deref(),
            Some("good")
        );
        assert_eq!(cmd("sh", &["-c", "printf bad; exit 1"]), None);
        let output = cmd(
            "sh",
            &[
                "-c",
                "i=0; while [ $i -lt 10000 ]; do printf 1234567890; i=$((i+1)); done",
            ],
        );
        assert_eq!(output.unwrap().len(), 100000);
    }

    #[test]
    fn command_deadline_covers_descendants_holding_stdout() {
        let start = std::time::Instant::now();
        assert_eq!(cmd("sh", &["-c", "sleep 30 &"]), None);
        assert_eq!(cmd("sh", &["-c", "exec sleep 30"]), None);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
    }

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

    #[test]
    fn ioreg_values_decode_all_three_shapes() {
        let dump = "\
+-o AGXAcceleratorG13G  <class AGXAcceleratorG13G>
    | \"model\" = <\"Apple M1\">
    | \"gpu-core-count\" = 8
    | \"board-id\" = <4d61636d696e69392c3100>
    | \"IOClass\" = \"AGXAcceleratorG13G\"
";
        // Printable CFData: <"str">.
        assert_eq!(ioreg_string(dump, "model").as_deref(), Some("Apple M1"));
        // Hex CFData holding a NUL-terminated ASCII string.
        assert_eq!(
            ioreg_string(dump, "board-id").as_deref(),
            Some("Macmini9,1")
        );
        // Plain CFString.
        assert_eq!(
            ioreg_string(dump, "IOClass").as_deref(),
            Some("AGXAcceleratorG13G")
        );
        // Bare integer, and a missing key.
        assert_eq!(ioreg_u64(dump, "gpu-core-count"), Some(8));
        assert_eq!(ioreg_u64(dump, "model"), None);
        assert_eq!(ioreg_string(dump, "absent"), None);
    }
}
