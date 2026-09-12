use std::path::{Path, PathBuf};

use wilios_cli::dump::{DumpFormat, DumpOpts, dump};
use wilios_cli::midi::{MidiOpts, export_midi};
use wilios_cli::pipeline::load_interpreter;
use wilios_cli::play::play;
use wilios_cli::progress::RenderBar;
use wilios_cli::render::{
    DEFAULT_MAX_RENDER_SECS, DEFAULT_SAMPLE_RATE, RenderOpts, render_with_progress,
};
use wilios_cli::smoke::{SmokeOpts, smoke};

const USAGE: &str = "\
Usage:
  wilios <file>                          play a .wilios file on the default audio device
  wilios play <file>                     same as above, explicit
  wilios render <file> -o OUT.wav [opts] render to a WAV file (no audio device)
  wilios midi <file> -o OUT.mid [opts]   export a Standard MIDI File (no audio device)
  wilios dump <file> [opts]              print the per-track event timeline to stdout
  wilios smoke <file> [opts]             headless check: schedules cleanly, tracks end together

render options:
  -o, --output <path>     output WAV path (required)
  --duration <seconds>    render exactly this long; required for endless-loop pieces
  --sample-rate <hz>      output sample rate (default 44100)
  --no-progress           do not draw the stderr progress bar

midi options:
  -o, --output <path>     output .mid path (required)
  --duration <seconds>    export exactly this much; required for endless-loop pieces

dump options:
  --format <json|text|roll>  output format (default text; roll = ASCII piano roll)
  --duration <seconds>       dump exactly this much; required for endless-loop pieces

smoke options:
  --duration <seconds>    schedule this much; skips the equal-end-time check
";

fn main() {
    // stderr, not stdout: this binary's logic is the template for wilios-mcp,
    // which reserves stdout for its own protocol framing. RUST_LOG (e.g.
    // `RUST_LOG=debug`) controls verbosity; defaults to info-level.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let rest = &args[1..];

    match rest.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            eprint!("{USAGE}");
            std::process::exit(if rest.is_empty() { 1 } else { 0 });
        }
        Some("render") => run_render(&rest[1..]),
        Some("midi") => run_midi(&rest[1..]),
        Some("dump") => run_dump(&rest[1..]),
        Some("smoke") => run_smoke(&rest[1..]),
        Some("play") => run_play(&single_file(&rest[1..])),
        // Backwards compatible: `wilios <file>` plays.
        Some(_) => run_play(&single_file(rest)),
    }
}

fn fail(msg: impl std::fmt::Display) -> ! {
    tracing::error!("{msg}");
    std::process::exit(1);
}

fn single_file(args: &[String]) -> PathBuf {
    match args {
        [f] => PathBuf::from(f),
        _ => fail(format_args!(
            "expected exactly one file argument\n\n{USAGE}"
        )),
    }
}

fn run_play(file: &Path) {
    let interp = load_interpreter(file).unwrap_or_else(|e| fail(e));
    if let Err(e) = play(interp) {
        fail(e);
    }
}

fn run_render(args: &[String]) {
    let mut file: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut duration: Option<f32> = None;
    let mut sample_rate: u32 = DEFAULT_SAMPLE_RATE;
    let mut show_progress = true;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(next_value(args, &mut i, "-o/--output"))),
            "--no-progress" => show_progress = false,
            "--duration" => {
                let v = next_value(args, &mut i, "--duration");
                let secs: f32 = v
                    .parse()
                    .unwrap_or_else(|_| fail(format!("invalid --duration '{v}'")));
                if !secs.is_finite() || secs <= 0.0 {
                    fail("--duration must be a finite number greater than 0");
                }
                duration = Some(secs);
            }
            "--sample-rate" => {
                let v = next_value(args, &mut i, "--sample-rate");
                let hz: u32 = v
                    .parse()
                    .unwrap_or_else(|_| fail(format!("invalid --sample-rate '{v}'")));
                if hz == 0 {
                    fail("--sample-rate must be greater than 0");
                }
                sample_rate = hz;
            }
            "-h" | "--help" => {
                eprint!("{USAGE}");
                std::process::exit(0);
            }
            other if other.starts_with('-') => fail(format!("unknown option '{other}'\n\n{USAGE}")),
            _ => {
                if file.replace(PathBuf::from(&args[i])).is_some() {
                    fail(format_args!("render takes exactly one file\n\n{USAGE}"));
                }
            }
        }
        i += 1;
    }

    let file = file.unwrap_or_else(|| fail(format_args!("render: missing <file>\n\n{USAGE}")));
    let out = out.unwrap_or_else(|| fail(format_args!("render: missing -o/--output\n\n{USAGE}")));

    let interp = load_interpreter(&file).unwrap_or_else(|e| fail(e));
    let opts = RenderOpts {
        out,
        sample_rate,
        duration,
        max_render_secs: DEFAULT_MAX_RENDER_SECS,
    };
    let mut bar = RenderBar::new(show_progress);
    let result = render_with_progress(interp, opts, &mut |p| bar.update(&p));
    bar.finish();
    if let Err(e) = result {
        fail(e);
    }
}

fn run_midi(args: &[String]) {
    let mut file: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut duration: Option<f32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(next_value(args, &mut i, "-o/--output"))),
            "--duration" => {
                let v = next_value(args, &mut i, "--duration");
                let secs: f32 = v
                    .parse()
                    .unwrap_or_else(|_| fail(format!("invalid --duration '{v}'")));
                if !secs.is_finite() || secs <= 0.0 {
                    fail("--duration must be a finite number greater than 0");
                }
                duration = Some(secs);
            }
            "-h" | "--help" => {
                eprint!("{USAGE}");
                std::process::exit(0);
            }
            other if other.starts_with('-') => fail(format!("unknown option '{other}'\n\n{USAGE}")),
            _ => {
                if file.replace(PathBuf::from(&args[i])).is_some() {
                    fail(format_args!("midi takes exactly one file\n\n{USAGE}"));
                }
            }
        }
        i += 1;
    }

    let file = file.unwrap_or_else(|| fail(format_args!("midi: missing <file>\n\n{USAGE}")));
    let out = out.unwrap_or_else(|| fail(format_args!("midi: missing -o/--output\n\n{USAGE}")));

    let interp = load_interpreter(&file).unwrap_or_else(|e| fail(e));
    let opts = MidiOpts {
        out,
        duration,
        max_secs: DEFAULT_MAX_RENDER_SECS,
    };
    if let Err(e) = export_midi(interp, opts) {
        fail(e);
    }
}

fn run_dump(args: &[String]) {
    let mut file: Option<PathBuf> = None;
    let mut format = DumpFormat::Text;
    let mut duration: Option<f32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                let v = next_value(args, &mut i, "--format");
                format = match v {
                    "json" => DumpFormat::Json,
                    "text" => DumpFormat::Text,
                    "roll" => DumpFormat::Roll,
                    other => fail(format!(
                        "invalid --format '{other}' (expected 'json', 'text', or 'roll')"
                    )),
                };
            }
            "--duration" => {
                let v = next_value(args, &mut i, "--duration");
                let secs: f32 = v
                    .parse()
                    .unwrap_or_else(|_| fail(format!("invalid --duration '{v}'")));
                if !secs.is_finite() || secs <= 0.0 {
                    fail("--duration must be a finite number greater than 0");
                }
                duration = Some(secs);
            }
            "-h" | "--help" => {
                eprint!("{USAGE}");
                std::process::exit(0);
            }
            other if other.starts_with('-') => fail(format!("unknown option '{other}'\n\n{USAGE}")),
            _ => {
                if file.replace(PathBuf::from(&args[i])).is_some() {
                    fail(format_args!("dump takes exactly one file\n\n{USAGE}"));
                }
            }
        }
        i += 1;
    }

    let file = file.unwrap_or_else(|| fail(format_args!("dump: missing <file>\n\n{USAGE}")));

    let interp = load_interpreter(&file).unwrap_or_else(|e| fail(e));
    let opts = DumpOpts {
        format,
        duration,
        max_secs: DEFAULT_MAX_RENDER_SECS,
    };
    match dump(interp, opts) {
        Ok(payload) => println!("{payload}"),
        Err(e) => fail(e),
    }
}

fn run_smoke(args: &[String]) {
    let mut file: Option<PathBuf> = None;
    let mut duration: Option<f32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--duration" => {
                let v = next_value(args, &mut i, "--duration");
                let secs: f32 = v
                    .parse()
                    .unwrap_or_else(|_| fail(format!("invalid --duration '{v}'")));
                if !secs.is_finite() || secs <= 0.0 {
                    fail("--duration must be a finite number greater than 0");
                }
                duration = Some(secs);
            }
            "-h" | "--help" => {
                eprint!("{USAGE}");
                std::process::exit(0);
            }
            other if other.starts_with('-') => fail(format!("unknown option '{other}'\n\n{USAGE}")),
            _ => {
                if file.replace(PathBuf::from(&args[i])).is_some() {
                    fail(format_args!("smoke takes exactly one file\n\n{USAGE}"));
                }
            }
        }
        i += 1;
    }

    let file = file.unwrap_or_else(|| fail(format_args!("smoke: missing <file>\n\n{USAGE}")));

    let interp = load_interpreter(&file).unwrap_or_else(|e| fail(e));
    let opts = SmokeOpts {
        duration,
        max_secs: DEFAULT_MAX_RENDER_SECS,
    };
    match smoke(interp, opts) {
        Ok(summary) => tracing::info!("{summary}"),
        Err(e) => fail(e),
    }
}

fn next_value<'a>(args: &'a [String], i: &mut usize, flag: &str) -> &'a str {
    *i += 1;
    args.get(*i)
        .map(String::as_str)
        .unwrap_or_else(|| fail(format!("{flag} requires a value")))
}
