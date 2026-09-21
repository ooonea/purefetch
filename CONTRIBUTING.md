# Contributing to purefetch

Thanks for your interest! purefetch aims to stay small, fast, and
**dependency-free**. Bug reports, ideas, and pull requests are all welcome.

## Ground rules

- **Zero dependencies.** No crates.io dependencies — the tool is `std` plus the
  platform layer in `src/sys/` (raw Linux syscalls; direct libSystem FFI on
  macOS). A PR that adds a dependency will be asked to drop it.
- **No panics, no blocking.** Detection modules read `/proc` and `/sys` on
  Linux (sysctl & co. via `crate::sys` on macOS) and must return an empty
  `Vec` when data is missing rather than `unwrap()`-ing or spawning
  long-running work.
- **MSRV is 1.70** — don't use newer `std` APIs (a dedicated CI job builds and
  tests on 1.70; `Cargo.lock` stays in the v3 format so pre-1.78 cargo can read it).
- Run `cargo fmt` and `cargo clippy` before submitting; CI enforces both.

## Building & checking

```sh
cargo build --release
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
python3 tests/cli.py ./target/release/purefetch
```

## Adding a distro logo

1. Create `assets/logos/<id>.txt`, where `<id>` matches the `/etc/os-release`
   `ID` (so auto-detection works). Format:

   ```
   COLORS: <sgr> <sgr> ...
   <ascii art, one row per line>
   ```

   The `COLORS:` line lists SGR parameter strings (e.g. `31 37`), one per color;
   in the art `$1`..`$9` switch to the Nth color, `$$` is a literal `$` (the
   fastfetch escape — fastfetch art files work verbatim), and any other `$` not
   followed by a digit 1-9 is also literal. Keep it ≤ 40 columns wide and
   ~16–20 rows, and preserve leading spaces.
2. Add `<id>` to the `ORDER` list in `examples/genlogos.rs` (with any aliases).
3. Regenerate: `cargo run --example genlogos` — it rewrites `src/logo.rs` and runs
   `rustfmt` on it. Don't edit `src/logo.rs` by hand.
4. Check it: `cargo run -- --logo <id>`.

## Adding an info module

Create `src/detect/<name>.rs` exposing `pub fn detect() -> crate::detect::Rows`,
then register it (label + function) in the `groups` table in `src/main.rs`. Use
`Row::val(...)` for a single value and return an empty `Vec` when unavailable.
`src/detect/cpu.rs` is the canonical example.

## Adding a CPU architecture (Linux)

`src/sys/linux.rs` issues raw syscalls per architecture. To add one, provide a
`syscall3` implementation (inline asm for that target's syscall convention) plus
the `SYS_STATFS` / `SYS_IOCTL` numbers, all behind `#[cfg(target_arch = "...")]`.
The `struct Statfs` layout is shared by LP64 targets; 32-bit targets would need
their own (`statfs64`) layout. Verify with `cargo check --target <triple>` and,
ideally, a run under `qemu-<arch>`.

## Adding an OS

One submodule in `src/sys/` implementing the API listed in `src/sys/mod.rs`,
plus a `#[cfg(target_os = "...")] pub fn detect()` per detection module that
differs. macOS (`src/sys/darwin.rs`) is the template: FFI structs mirror the
OS's C headers and every helper degrades to `None`/empty instead of failing.
Keep pure parsing helpers unconditional so the tests cover them on every host.

## Commit & PR style

- Small, focused commits with a `<area>: <summary>` subject line.
- By submitting a pull request you agree that your contribution is dual-licensed
  under **MIT OR Apache-2.0**, per the terms in the README.
