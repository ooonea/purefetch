//! purefetch — system information, written entirely in Rust (zero deps).

mod color;
mod detect;
mod logo;
mod render;
mod sys;
mod util;

use render::Line;

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

type Det = fn() -> detect::Rows;

/// One info line source: a built-in detector, or a custom `--exec` command.
enum Mod {
    Builtin(&'static str, Det),
    Exec(String, String), // (label, shell command)
}

fn main() {
    let cli: Vec<String> = std::env::args().skip(1).collect();

    // Pre-scan for the config controls so we know what to read.
    let mut explicit_config: Option<String> = None;
    let mut no_config = false;
    {
        let mut j = 0;
        while j < cli.len() {
            match cli[j].as_str() {
                "--config" => {
                    j += 1;
                    explicit_config = cli.get(j).cloned();
                }
                "--no-config" => no_config = true,
                _ => {}
            }
            j += 1;
        }
    }

    // Config-file options come first; real CLI args are appended so they win.
    let mut args: Vec<String> = Vec::new();
    if let Some(path) = config_path(explicit_config, no_config) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            args.extend(config_to_args(&text));
        }
    }
    args.extend(cli);

    let mut logo_sel = String::from("auto");
    let mut logo_file: Option<String> = None;
    let mut logo_exec: Option<String> = None;
    let mut modules_arg: Option<String> = None;
    let mut execs: Vec<(String, String)> = Vec::new();
    let mut no_color = false;
    let mut no_color_blocks = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-V" | "--version" => {
                println!("{NAME} {VERSION}");
                return;
            }
            // Config controls: resolved in the pre-scan above; skip here.
            "--no-config" => {}
            "--config" => i += 1,
            "--no-color" | "--no-colour" => no_color = true,
            "--no-color-blocks" => no_color_blocks = true,
            "--no-logo" => logo_sel = "none".into(),
            "-l" | "--logo" => {
                i += 1;
                match args.get(i) {
                    Some(v) => logo_sel = v.clone(),
                    None => fail("--logo requires a value"),
                }
            }
            "--logo-file" => {
                i += 1;
                match args.get(i) {
                    Some(v) => logo_file = Some(v.clone()),
                    None => fail("--logo-file requires a path"),
                }
            }
            "--logo-exec" => {
                i += 1;
                match args.get(i) {
                    Some(v) => logo_exec = Some(v.clone()),
                    None => fail("--logo-exec requires a command"),
                }
            }
            "--modules" => {
                i += 1;
                match args.get(i) {
                    Some(v) => modules_arg = Some(v.clone()),
                    None => fail("--modules requires a value"),
                }
            }
            "--exec" => {
                i += 1;
                match args.get(i) {
                    Some(v) => match v.split_once(':') {
                        Some((label, cmd)) => {
                            execs.push((label.trim().to_string(), cmd.to_string()))
                        }
                        None => fail("--exec expects \"Label:command\""),
                    },
                    None => fail("--exec requires a value"),
                }
            }
            other => {
                eprintln!("{NAME}: unknown option '{other}' (try --help)");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let tty = sys::stdout_is_tty();
    let color_enabled = !no_color && std::env::var_os("NO_COLOR").is_none() && tty;
    let pal = color::Palette::new(color_enabled);
    let term_width = if tty { sys::term_width() } else { 0 };

    // Logo source, in precedence order: --logo-file (verbatim file) >
    // --logo-exec (command output, verbatim) > built-in / auto-detected.
    let logo_lines: Vec<String> = if let Some(path) = &logo_file {
        verbatim_logo(
            &std::fs::read_to_string(path).unwrap_or_default(),
            color_enabled,
        )
    } else if let Some(cmd) = &logo_exec {
        verbatim_logo(&util::sh_raw(cmd).unwrap_or_default(), color_enabled)
    } else {
        match logo::get(&logo_sel) {
            Some(l) => l.lines.iter().map(|ln| pal.paint(l.sgr, ln)).collect(),
            None => Vec::new(),
        }
    };

    // Title: user@host.
    let user = std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("LOGNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "user".into());
    let host = util::read_trim("/proc/sys/kernel/hostname").unwrap_or_else(|| "localhost".into());
    let title_plain = format!("{user}@{host}");
    let sep_len = title_plain.chars().count();

    let sep_line = || Line::Raw(pal.paint(pal.sep, &"─".repeat(sep_len)));

    // Info modules: the caller-selected set (--modules) or the default layout.
    let groups = match &modules_arg {
        Some(list) => parse_modules(list, &execs),
        None => default_groups(),
    };

    let mut lines: Vec<Line> = vec![Line::Raw(pal.paint(pal.title, &title_plain))];

    for group in &groups {
        let mut produced: Vec<Line> = Vec::new();
        for m in group {
            match m {
                Mod::Builtin(label, det) => {
                    for row in (*det)() {
                        let key = row.key.unwrap_or_else(|| (*label).to_string());
                        produced.push(Line::Kv(key, row.value));
                    }
                }
                Mod::Exec(label, cmd) => {
                    if let Some(val) = util::sh(cmd) {
                        produced.push(Line::Kv(label.clone(), val));
                    }
                }
            }
        }
        if !produced.is_empty() {
            lines.push(sep_line());
            lines.extend(produced);
        }
    }

    if color_enabled && !no_color_blocks {
        lines.push(sep_line());
        lines.push(Line::Raw(color_blocks(false)));
        lines.push(Line::Raw(color_blocks(true)));
    }

    render::render(&logo_lines, &lines, &pal, term_width);
}

/// Split a raw (possibly ANSI) logo into lines, stripping ANSI when color is off.
fn verbatim_logo(raw: &str, color_enabled: bool) -> Vec<String> {
    raw.lines()
        .map(|ln| {
            if color_enabled {
                ln.to_string()
            } else {
                render::strip_ansi(ln)
            }
        })
        .collect()
}

fn fail(msg: &str) -> ! {
    eprintln!("{NAME}: {msg}");
    std::process::exit(2);
}

/// Locate the config file: explicit `--config`, then `$PUREFETCH_CONFIG`, then
/// `$XDG_CONFIG_HOME/purefetch/config` (or `~/.config/...`), then `/etc/purefetch/config`.
fn config_path(explicit: Option<String>, no_config: bool) -> Option<String> {
    if no_config {
        return None;
    }
    if explicit.is_some() {
        return explicit;
    }
    if let Ok(p) = std::env::var("PUREFETCH_CONFIG") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    let user = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|b| format!("{b}/purefetch/config"))
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| format!("{h}/.config/purefetch/config"))
        });
    if let Some(p) = user {
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    let etc = "/etc/purefetch/config";
    if std::path::Path::new(etc).exists() {
        return Some(etc.to_string());
    }
    None
}

/// Turn a config file into pseudo CLI args. Each non-comment line is
/// `<option> [value]`, e.g. `modules os,cpu` -> ["--modules", "os,cpu"].
fn config_to_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, rest) = match line.split_once(char::is_whitespace) {
            Some((k, r)) => (k, r.trim()),
            None => (line, ""),
        };
        out.push(format!("--{key}"));
        if !rest.is_empty() {
            out.push(rest.to_string());
        }
    }
    out
}

/// The default module layout, grouped by blank separators.
fn default_groups() -> Vec<Vec<Mod>> {
    let g = |v: &[(&'static str, Det)]| {
        v.iter()
            .map(|&(l, d)| Mod::Builtin(l, d))
            .collect::<Vec<_>>()
    };
    vec![
        g(&[
            ("OS", detect::os::detect as Det),
            ("Host", detect::host::detect),
            ("Kernel", detect::kernel::detect),
            ("Uptime", detect::uptime::detect),
            ("Packages", detect::packages::detect),
            ("Shell", detect::shell::detect),
            ("Display", detect::display::detect),
            ("DE", detect::de::detect),
            ("WM", detect::wm::detect),
            ("Terminal", detect::terminal::detect),
        ]),
        g(&[
            ("CPU", detect::cpu::detect as Det),
            ("GPU", detect::gpu::detect),
            ("Memory", detect::memory::detect),
            ("Swap", detect::swap::detect),
            ("Disk (/)", detect::disk::detect),
        ]),
        g(&[
            ("Locale", detect::locale::detect as Det),
            ("Battery", detect::battery::detect),
        ]),
    ]
}

/// Parse a `--modules` list into groups. Items are built-in module names or the
/// (lowercased) label of an `--exec` module; `-` (or `sep`) starts a new group.
fn parse_modules(list: &str, execs: &[(String, String)]) -> Vec<Vec<Mod>> {
    let mut groups: Vec<Vec<Mod>> = Vec::new();
    let mut cur: Vec<Mod> = Vec::new();
    for item in list.split(',') {
        let it = item.trim();
        if it.is_empty() {
            continue;
        }
        if it == "-" || it.eq_ignore_ascii_case("sep") {
            if !cur.is_empty() {
                groups.push(std::mem::take(&mut cur));
            }
        } else if let Some((label, det)) = builtin_by_name(it) {
            cur.push(Mod::Builtin(label, det));
        } else if let Some((label, cmd)) = execs.iter().find(|(l, _)| l.eq_ignore_ascii_case(it)) {
            cur.push(Mod::Exec(label.clone(), cmd.clone()));
        }
    }
    if !cur.is_empty() {
        groups.push(cur);
    }
    groups
}

/// Map a built-in module name (case-insensitive) to its (default label, detector).
fn builtin_by_name(name: &str) -> Option<(&'static str, Det)> {
    Some(match name.to_ascii_lowercase().as_str() {
        "os" => ("OS", detect::os::detect as Det),
        "host" => ("Host", detect::host::detect),
        "kernel" => ("Kernel", detect::kernel::detect),
        "uptime" => ("Uptime", detect::uptime::detect),
        "packages" => ("Packages", detect::packages::detect),
        "shell" => ("Shell", detect::shell::detect),
        "display" => ("Display", detect::display::detect),
        "de" => ("DE", detect::de::detect),
        "wm" => ("WM", detect::wm::detect),
        "terminal" => ("Terminal", detect::terminal::detect),
        "cpu" => ("CPU", detect::cpu::detect),
        "gpu" => ("GPU", detect::gpu::detect),
        "memory" | "ram" => ("Memory", detect::memory::detect),
        "swap" => ("Swap", detect::swap::detect),
        "disk" => ("Disk (/)", detect::disk::detect),
        "locale" => ("Locale", detect::locale::detect),
        "battery" => ("Battery", detect::battery::detect),
        _ => return None,
    })
}

/// A row of eight ANSI color blocks (normal or bright).
fn color_blocks(bright: bool) -> String {
    let base = if bright { 100 } else { 40 };
    let mut s = String::new();
    for c in 0..8 {
        s.push_str(&format!("\x1b[{}m   ", base + c));
    }
    s.push_str("\x1b[0m");
    s
}

fn print_help() {
    println!("{NAME} {VERSION} — system information, written entirely in Rust");
    println!();
    println!("USAGE:");
    println!("    {NAME} [OPTIONS]");
    println!();
    println!("Options may also be set, one per line as `key value`, in a config file:");
    println!("$PUREFETCH_CONFIG, ~/.config/purefetch/config, or /etc/purefetch/config.");
    println!();
    println!("OPTIONS:");
    println!("    -l, --logo <NAME>       logo: auto (default), a distro name, tux, or none");
    println!("        --logo-file <PATH>  use a custom logo, read verbatim from a file");
    println!("        --logo-exec <CMD>   use a custom logo from a command's output (dynamic)");
    println!("        --modules <LIST>    comma-separated modules to show ('-' = separator),");
    println!("                            e.g. os,host,kernel,-,cpu,gpu,memory,swap,-,shell");
    println!("        --exec <LABEL:CMD>  add a custom line running a shell command; refer to");
    println!("                            it in --modules by <label> (lowercased). Repeatable.");
    println!("        --config <PATH>     read options from PATH");
    println!("        --no-config         ignore any config file");
    println!("        --no-logo           do not print any logo");
    println!("        --no-color          disable ANSI colors");
    println!("        --no-color-blocks   hide the trailing ANSI color blocks");
    println!("    -V, --version           print version and exit");
    println!("    -h, --help              print this help and exit");
}
