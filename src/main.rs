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

fn main() {
    let mut logo_sel = String::from("auto");
    let mut no_color = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
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
            "--no-color" | "--no-colour" => no_color = true,
            "--no-logo" => logo_sel = "none".into(),
            "-l" | "--logo" => {
                i += 1;
                match args.get(i) {
                    Some(v) => logo_sel = v.clone(),
                    None => {
                        eprintln!("{NAME}: --logo requires a value");
                        std::process::exit(2);
                    }
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

    // Logo (already colored).
    let logo_lines: Vec<String> = match logo::get(&logo_sel) {
        Some(l) => l.lines.iter().map(|ln| pal.paint(l.sgr, ln)).collect(),
        None => Vec::new(),
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

    // Ordered module groups. Each entry: (default label, detect fn).
    type Det = fn() -> detect::Rows;
    let groups: Vec<Vec<(&str, Det)>> = vec![
        vec![
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
        ],
        vec![
            ("CPU", detect::cpu::detect as Det),
            ("GPU", detect::gpu::detect),
            ("Memory", detect::memory::detect),
            ("Swap", detect::swap::detect),
            ("Disk (/)", detect::disk::detect),
        ],
        vec![
            ("Locale", detect::locale::detect as Det),
            ("Battery", detect::battery::detect),
        ],
    ];

    let mut lines: Vec<Line> = vec![Line::Raw(pal.paint(pal.title, &title_plain))];

    for group in &groups {
        let mut produced: Vec<Line> = Vec::new();
        for &(label, det) in group {
            for row in det() {
                let key = row.key.unwrap_or_else(|| label.to_string());
                produced.push(Line::Kv(key, row.value));
            }
        }
        if !produced.is_empty() {
            lines.push(sep_line());
            lines.extend(produced);
        }
    }

    if color_enabled {
        lines.push(sep_line());
        lines.push(Line::Raw(color_blocks(false)));
        lines.push(Line::Raw(color_blocks(true)));
    }

    render::render(&logo_lines, &lines, &pal, term_width);
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
    println!("OPTIONS:");
    println!("    -l, --logo <NAME>   logo: auto (default), a distro name, tux, or none");
    println!("        --no-logo       do not print any logo");
    println!("        --no-color      disable ANSI colors");
    println!("    -V, --version       print version and exit");
    println!("    -h, --help          print this help and exit");
}
