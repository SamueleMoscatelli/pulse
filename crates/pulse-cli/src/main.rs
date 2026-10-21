//! `pulse`: static analyzer and Quality Gate for AI assets.
#![allow(clippy::print_stdout)]

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, IsTerminal as _, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use pulse_core::config::CONFIG_FILE;
use pulse_core::{Config, DEFAULT_TOML, Severity};
use pulse_engine::{GateStatus, Report, ScanOptions};
use pulse_report::{
    JsonOptions, SarifOptions, TerminalOptions, render_json, render_sarif, render_terminal,
};

/// Exit code for a failed Quality Gate.
const EXIT_GATE: u8 = 1;
/// Exit code for usage, config or I/O errors.
const EXIT_USAGE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "pulse",
    version,
    about = "Static analyzer and Quality Gate for AI assets"
)]
struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct Global {
    /// Scan root
    #[arg(
        short = 'C',
        long = "root",
        global = true,
        default_value = ".",
        value_name = "DIR"
    )]
    root: PathBuf,
    /// Config path (default: <root>/pulse.toml if present)
    #[arg(short = 'c', long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    /// When to use colors (also honors NO_COLOR)
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,
    /// Findings only, no summary
    #[arg(short, long, global = true)]
    quiet: bool,
    /// Show the remediation of every finding
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Print phase timings to stderr
    #[arg(long, global = true)]
    timings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Terminal,
    Json,
    Sarif,
    Markdown,
    Github,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a starter pulse.toml
    Init(InitArgs),
    /// Analyze AI assets and print findings
    Scan(ScanArgs),
    /// Scan and enforce the Quality Gate (= scan --fail-on-gate)
    Ci(ScanArgs),
    /// Manage the findings baseline
    Baseline,
    /// List available rules
    Rules,
    /// Show the documentation of a rule
    Explain,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Scan(_) => "scan",
            Self::Ci(_) => "ci",
            Self::Baseline => "baseline",
            Self::Rules => "rules",
            Self::Explain => "explain",
        }
    }
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Overwrite an existing pulse.toml
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct ScanArgs {
    /// Restrict findings to these files or directories (relative to the root)
    paths: Vec<PathBuf>,
    /// Report format [default: terminal]
    #[arg(short, long, value_enum)]
    format: Option<Format>,
    /// Write the report to a file instead of stdout
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Exit 1 when the Quality Gate fails
    #[arg(long)]
    fail_on_gate: bool,
    /// Hide findings below this level: info, low, medium, high, critical (display only)
    #[arg(long, value_name = "LEVEL", default_value = "info", value_parser = Severity::from_str)]
    min_severity: Severity,
    /// Run only rules with this ID prefix (repeatable)
    #[arg(long = "rule", value_name = "ID")]
    rules: Vec<String>,
    /// Skip rules with this ID prefix (repeatable)
    #[arg(long = "skip-rule", value_name = "ID")]
    skip_rules: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Init(args) => init(&cli.global.root, args.force).map(|path| {
            println!("created {}", path.display());
            ExitCode::SUCCESS
        }),
        Command::Scan(args) => run_scan(&cli.global, args, false),
        Command::Ci(args) => run_scan(&cli.global, args, true),
        other => Err(anyhow::anyhow!(
            "pulse {}: not implemented yet",
            other.name()
        )),
    };
    result.unwrap_or_else(|e| {
        eprintln!("error: {e:#}");
        ExitCode::from(EXIT_USAGE)
    })
}

/// `scan` and `ci` (`ci` implies `--fail-on-gate` and its own format/color defaults).
fn run_scan(global: &Global, args: &ScanArgs, ci: bool) -> anyhow::Result<ExitCode> {
    let github_actions = std::env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true");
    let format = args.format.unwrap_or(if ci && github_actions {
        Format::Github
    } else {
        Format::Terminal
    });
    match format {
        // `github` renders as terminal until its reporter lands (P6.2).
        Format::Terminal | Format::Github | Format::Json | Format::Sarif => {}
        other => bail!(
            "format `{}` is not implemented yet",
            other
                .to_possible_value()
                .map_or_else(String::new, |v| v.get_name().to_owned())
        ),
    }

    let root = &global.root;
    let config = Config::load(root, global.config.as_deref())?;
    let opts = ScanOptions {
        paths: args.paths.iter().map(|p| scan_path(root, p)).collect(),
        only_rules: args.rules.clone(),
        skip_rules: args.skip_rules.clone(),
    };
    let report = pulse_engine::scan(root, &config, &opts)?;
    for (path, reason) in &report.walk_errors {
        eprintln!("warning: skipped {}: {reason}", path.display());
    }
    if global.timings {
        eprintln!("{}", timings(&report));
    }

    let to_file = args.output.is_some();
    let tty = !to_file && io::stdout().is_terminal();
    let term = TerminalOptions {
        root,
        // `ci` keeps color off unless forced: logs are rarely terminals.
        color: use_color(global.color, tty && !ci, no_color()),
        quiet: global.quiet,
        verbose: global.verbose,
        min_severity: args.min_severity,
        window: config.context.window,
    };
    let write = |w: &mut dyn Write| -> io::Result<()> {
        match format {
            Format::Json => {
                let opts = JsonOptions {
                    root,
                    min_severity: args.min_severity,
                    generated_at: SystemTime::now(),
                };
                render_json(w, &report, &opts)?;
            }
            Format::Sarif => {
                let opts = SarifOptions {
                    root,
                    min_severity: args.min_severity,
                };
                render_sarif(w, &report, &opts)?;
            }
            _ => render_terminal(w, &report, &term)?,
        }
        w.flush()
    };
    let written = match &args.output {
        Some(path) => {
            let file =
                File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
            write(&mut BufWriter::new(file))
                .with_context(|| format!("cannot write {}", path.display()))
        }
        None => match write(&mut BufWriter::new(io::stdout().lock())) {
            // A closed pipe (`pulse scan | head`) is not an error.
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            other => other.context("cannot write to stdout"),
        },
    };
    written?;

    let gate_failed = report.gate.status == GateStatus::Failed;
    Ok(if (ci || args.fail_on_gate) && gate_failed {
        ExitCode::from(EXIT_GATE)
    } else {
        ExitCode::SUCCESS
    })
}

/// Whether to emit ANSI styles; an explicit `--color` wins over `NO_COLOR`.
fn use_color(choice: ColorChoice, tty: bool, no_color: bool) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => tty && !no_color,
    }
}

/// `NO_COLOR` is set to a non-empty value (no-color.org).
fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// A `PATHS` argument as the root-relative, `/`-separated form the engine expects.
fn scan_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn timings(report: &Report) -> String {
    let ms = |d: Duration| format!("{:.2} ms", d.as_secs_f64() * 1e3);
    let t = &report.timings;
    format!(
        "timings: discover {} · read {} · rules {} · total {}",
        ms(t.discover),
        ms(t.read),
        ms(t.rules),
        ms(t.total)
    )
}

/// Writes the default config to `<root>/pulse.toml`; refuses to overwrite unless `force`.
fn init(root: &Path, force: bool) -> anyhow::Result<PathBuf> {
    let path = root.join(CONFIG_FILE);
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = match options.open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            bail!(
                "{} already exists (use --force to overwrite)",
                path.display()
            )
        }
        Err(e) => return Err(e).with_context(|| format!("cannot create {}", path.display())),
    };
    file.write_all(DEFAULT_TOML.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn init_writes_default_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = init(dir.path(), false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_TOML);
    }

    #[test]
    fn init_refuses_overwrite_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        std::fs::write(&path, "# mine\n").unwrap();
        let err = init(dir.path(), false).unwrap_err();
        assert!(err.to_string().contains("--force"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# mine\n");
        init(dir.path(), true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_TOML);
    }

    #[test]
    fn parses_init_flags() {
        let cli = Cli::try_parse_from(["pulse", "init", "-C", "x", "--force"]).unwrap();
        let Command::Init(args) = cli.command else {
            panic!("expected init");
        };
        assert_eq!(cli.global.root, PathBuf::from("x"));
        assert!(args.force);
    }

    #[test]
    fn parses_every_subcommand() {
        for name in ["init", "scan", "ci", "baseline", "rules", "explain"] {
            let cli = Cli::try_parse_from(["pulse", name]).unwrap();
            assert_eq!(cli.command.name(), name);
        }
    }

    #[test]
    fn parses_scan_flags() {
        let cli = Cli::try_parse_from([
            "pulse",
            "-q",
            "scan",
            "a",
            "b/",
            "--min-severity",
            "HIGH",
            "--rule",
            "AI-SEC",
            "--rule",
            "AI-STR",
            "--skip-rule",
            "AI-SEC-001",
            "--fail-on-gate",
            "-f",
            "github",
            "--color",
            "never",
            "-C",
            "r",
        ])
        .unwrap();
        let Command::Scan(args) = cli.command else {
            panic!("expected scan");
        };
        assert!(cli.global.quiet);
        assert_eq!(cli.global.color, ColorChoice::Never);
        assert_eq!(cli.global.root, PathBuf::from("r"));
        assert_eq!(args.paths, [PathBuf::from("a"), PathBuf::from("b/")]);
        assert_eq!(args.min_severity, Severity::High);
        assert_eq!(args.rules, ["AI-SEC", "AI-STR"]);
        assert_eq!(args.skip_rules, ["AI-SEC-001"]);
        assert!(args.fail_on_gate);
        assert_eq!(args.format, Some(Format::Github));
    }

    #[test]
    fn rejects_unknown_severity() {
        assert!(Cli::try_parse_from(["pulse", "scan", "--min-severity", "huge"]).is_err());
    }

    #[test]
    fn color_auto_needs_tty_and_no_no_color() {
        assert!(use_color(ColorChoice::Auto, true, false));
        assert!(!use_color(ColorChoice::Auto, false, false));
        assert!(!use_color(ColorChoice::Auto, true, true));
        assert!(use_color(ColorChoice::Always, false, true));
        assert!(!use_color(ColorChoice::Never, true, false));
    }

    #[test]
    fn scan_paths_are_root_relative() {
        let root = Path::new("/r");
        assert_eq!(scan_path(root, Path::new("/r/a/b.md")), "a/b.md");
        assert_eq!(scan_path(root, Path::new("a\\b.md")), "a/b.md");
    }
}
