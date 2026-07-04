# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.10] - 2026-07-05

### Added
- Multi-color logos: a logo's `.txt` can define several colors (a `COLORS:` line
  of SGR params) selected by `$1`..`$9` markers in the art, fastfetch-style, so
  distro logos render in their real multi-tone colors. A literal `$` is any `$`
  not followed by a digit, so the Debian swirl still renders correctly.
- Unit tests for the pure parsing/formatting helpers (IEC/percent formatting,
  ANSI width & strip, lspci / cpuinfo parsing, ppid & uptime & DRM-connector
  parsing, config-line and logo-marker handling).

### Changed
- All bundled distribution logos are now taken from fastfetch (which builds on
  neofetch's set) and render in their native colors, replacing the previous mix
  of adapted single-color art. Attribution and the MIT notices for both projects
  are in [CREDITS.md](CREDITS.md).

## [0.1.9] - 2026-07-04

### Fixed
- `--no-logo` / `--logo none` now suppress a logo configured via `--logo-file` /
  `--logo-exec` (e.g. from the config file). Logo sources are resolved by argument
  order, so a command-line flag overrides one coming from the config.
- GPU (lspci fallback): AMD adapters showed the `[AMD/ATI]` vendor tag instead of
  the model. Drop the vendor tag and a trailing `(rev …)`, so the marketing name
  (e.g. `Radeon RX 7900 XTX`, `Raphael`) is shown. NVIDIA/Intel unaffected.
- KDE Plasma version: prefer the full `plasmashell --version` (e.g. `6.2.4`) over
  the major-only `KDE_SESSION_VERSION`, which is now only a fallback.
- `--config` with no path now errors, consistent with the other value-taking flags.
- No more trailing whitespace on logo-only output rows.

### Changed
- A standalone `--exec` (without `--modules`) now appends its line to the default
  layout instead of being silently ignored.

## [0.1.8] - 2026-07-04

### Fixed
- Memory: report `MemTotal - MemAvailable` (matching `free`/htop). The previous
  ZFS-ARC subtraction under-reported used memory — the ARC genuinely occupies RAM.
- Disk on a ZFS root: report the whole pool via `zpool list` (labelled
  `Disk (<pool>)`) instead of only the root dataset's few GiB.

### Changed
- Logos: `nixos` and `manjaro` are now pure ASCII (were solid block-glyph art).
  All 24 bundled logos are strictly 7-bit ASCII.

## [0.1.7] - 2026-07-04

### Added
- Config file: any option can be set (one `key value` per line) in
  `$PUREFETCH_CONFIG`, `~/.config/purefetch/config`, or `/etc/purefetch/config`,
  so plain `purefetch` reproduces a custom setup. `--config` / `--no-config`.
- `--logo-exec <CMD>`: use a custom logo from a command's output, regenerated
  each run (e.g. for status-driven logos).

## [0.1.6] - 2026-07-04

### Added
- `--exec <LABEL:command>`: add a custom info line from a shell command, placed in
  `--modules` by its (lowercased) label. Repeatable.

## [0.1.5] - 2026-07-04

### Added
- `--modules <LIST>`: choose which info modules to show, in order, with `-` for a
  separator (e.g. `os,host,kernel,-,cpu,gpu,memory,swap,-,shell`).

## [0.1.4] - 2026-07-04

### Added
- `--logo-file <PATH>`: use a custom logo read verbatim from a file (ANSI and
  all), for fastfetch-style custom logos.
- `--no-color-blocks`: hide the trailing ANSI color blocks.

## [0.1.3] - 2026-07-04

### Changed
- Rewrote the logo generator in Rust (`examples/genlogos.rs`) and dropped the
  Python tooling, so the project is 100% Rust again.

## [0.1.2] - 2026-07-04

### Fixed
- Distro logos rendered in the wrong color: the logo generator emitted an
  incomplete SGR (missing the `38;2;` truecolor prefix), so every logo appeared
  washed out instead of its brand color. Affected 0.1.0 and 0.1.1.

## [0.1.1] - 2026-07-04

### Added
- More bundled distro logos: gentoo, endeavouros, kali, elementary, zorin,
  artix, rocky, almalinux, centos, devuan, mx, garuda.
- `riscv64` and `loongarch64` support (raw syscalls), joining x86_64 and aarch64.

### Fixed
- CPU detection on riscv (and ppc / some ARM SoCs), which lack a `model name`
  field in `/proc/cpuinfo` — fall back to `uarch` / `isa` / `cpu` / `Hardware`.

## [0.1.0] - 2026-07-04

Initial release.

### Added
- fastfetch-style system information with a distro logo, written entirely in Rust
  with **zero external dependencies** (`std` + raw x86_64/aarch64 Linux syscalls).
- Info modules: OS, Host, Kernel, Uptime, Packages, Shell, Display, DE, WM,
  Terminal, CPU, GPU, Memory, Swap, Disk, Locale, Battery, plus the color blocks.
- ZFS-aware memory usage (subtracts the reclaimable ARC).
- Bundled logos: debian, arch, ubuntu, fedora, mint, manjaro, pop, opensuse,
  alpine, void, nixos, tux — auto-detected from `/etc/os-release`.
- CLI: `--logo`, `--no-logo`, `--no-color`, `--version`, `--help`.
- Dual-licensed MIT OR Apache-2.0.

[Unreleased]: https://github.com/ooonea/purefetch/compare/v0.1.10...HEAD
[0.1.10]: https://github.com/ooonea/purefetch/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/ooonea/purefetch/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/ooonea/purefetch/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/ooonea/purefetch/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/ooonea/purefetch/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/ooonea/purefetch/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/ooonea/purefetch/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ooonea/purefetch/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ooonea/purefetch/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ooonea/purefetch/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ooonea/purefetch/releases/tag/v0.1.0
