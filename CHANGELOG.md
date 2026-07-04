# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/ooonea/purefetch/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/ooonea/purefetch/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ooonea/purefetch/releases/tag/v0.1.0
