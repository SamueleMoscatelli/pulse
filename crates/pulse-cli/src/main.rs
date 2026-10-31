//! `pulse`: static analyzer and Quality Gate for AI assets.
#![allow(clippy::print_stdout)]

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, IsTerminal as _, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use pulse_core::config::{CONFIG_FILE, did_you_mean};
use pulse_core::{Assistant, Category, Config, DEFAULT_TOML, Severity};
use pulse_engine::{
    BASELINE_FILE, Baseline, GateStatus, Report, RuleStatus, ScanOptions, category_of,
};
use pulse_report::{
    GithubOptions, JsonOptions, MarkdownOptions, SarifOptions, TerminalOptions, render_github,
    render_json, render_markdown, render_sarif, render_terminal,
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
    /// Show the fix for every finding (by default only for medium and higher)
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
    /// Write the current findings' fingerprints to the baseline
    Baseline(BaselineArgs),
    /// List available rules
    Rules(RulesArgs),
    /// Show the documentation of a rule
    Explain(ExplainArgs),
}

#[cfg(test)]
impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Scan(_) => "scan",
            Self::Ci(_) => "ci",
            Self::Baseline(_) => "baseline",
            Self::Rules(_) => "rules",
            Self::Explain(_) => "explain",
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
struct BaselineArgs {
    /// Baseline file [default: <root>/.pulse/baseline.json]
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RulesFormat {
    Terminal,
    Json,
    Markdown,
}

#[derive(Debug, Args)]
struct RulesArgs {
    /// Only this category: security, structure, efficiency, cognitive (or SEC, STR, EFF, COG)
    #[arg(long, value_name = "CAT", value_parser = parse_category)]
    category: Option<Category>,
    /// Output format
    #[arg(short, long, value_enum, default_value_t = RulesFormat::Terminal)]
    format: RulesFormat,
}

#[derive(Debug, Args)]
struct ExplainArgs {
    /// Rule ID, e.g. AI-SEC-001
    #[arg(value_name = "RULE-ID")]
    id: String,
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
    /// Only report findings not in the baseline (.pulse/baseline.json)
    #[arg(long)]
    new_only: bool,
    /// Only analyze assets changed since this git ref (plus untracked files)
    #[arg(long, value_name = "GIT-REF")]
    changed_since: Option<String>,
    /// Only scan files this assistant loads (repeatable or comma-separated): copilot,
    /// claude-code, cursor, windsurf, cline, gemini-cli, codex, kiro [default: every assistant]
    #[arg(long = "assistant", value_name = "NAME", value_delimiter = ',', value_parser = parse_assistant)]
    assistants: Vec<Assistant>,
}

fn parse_assistant(s: &str) -> Result<Assistant, String> {
    Assistant::from_id(s).ok_or_else(|| {
        let ids: Vec<&str> = Assistant::ALL.iter().map(|a| a.id()).collect();
        format!("expected one of {}", ids.join(", "))
    })
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
        Command::Baseline(args) => write_baseline(&cli.global, args).map(|(path, n)| {
            println!("wrote {n} fingerprints to {}", path.display());
            ExitCode::SUCCESS
        }),
        Command::Rules(args) => list_rules(&cli.global, args),
        Command::Explain(args) => explain(&cli.global, &args.id),
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
    let root = &global.root;
    let config = Config::load(root, global.config.as_deref())?;
    let baseline_path = root.join(BASELINE_FILE);
    let baseline = Baseline::load(&baseline_path)?;
    if baseline.is_none() && (args.new_only || config.gate.new_code_only) {
        eprintln!(
            "warning: no baseline at {}; every finding is new (run `pulse baseline`)",
            baseline_path.display()
        );
    }
    let changed = match &args.changed_since {
        Some(git_ref) => Some(changed_files(root, git_ref)?),
        None => None,
    };
    let opts = ScanOptions {
        paths: args.paths.iter().map(|p| scan_path(root, p)).collect(),
        only_rules: args.rules.clone(),
        skip_rules: args.skip_rules.clone(),
        changed,
        baseline,
        new_only: args.new_only,
        assistants: (!args.assistants.is_empty())
            .then(|| args.assistants.iter().copied().collect()),
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
        width: tty
            .then(terminal_size::terminal_size)
            .flatten()
            .map(|(w, _)| usize::from(w.0)),
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
            Format::Markdown => {
                let opts = MarkdownOptions {
                    root,
                    min_severity: args.min_severity,
                };
                render_markdown(w, &report, &opts)?;
            }
            Format::Github => {
                let opts = GithubOptions {
                    root,
                    min_severity: args.min_severity,
                };
                render_github(w, &report, &opts)?;
            }
            Format::Terminal => render_terminal(w, &report, &term)?,
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
    if format == Format::Github {
        append_step_summary(&report, root, args.min_severity)?;
    }

    let gate_failed = report.gate.status == GateStatus::Failed;
    Ok(if (ci || args.fail_on_gate) && gate_failed {
        ExitCode::from(EXIT_GATE)
    } else {
        ExitCode::SUCCESS
    })
}

/// `baseline`: scans everything and writes the fingerprints; returns the path and count.
fn write_baseline(global: &Global, args: &BaselineArgs) -> anyhow::Result<(PathBuf, usize)> {
    let root = &global.root;
    let config = Config::load(root, global.config.as_deref())?;
    let report = pulse_engine::scan(root, &config, &ScanOptions::default())?;
    let baseline = Baseline::from_findings(&report.findings);
    let path = args
        .output
        .clone()
        .unwrap_or_else(|| root.join(BASELINE_FILE));
    baseline.write(&path)?;
    Ok((path, baseline.len()))
}

/// Root-relative paths changed between `git_ref` and `HEAD`, plus untracked files.
fn changed_files(root: &Path, git_ref: &str) -> anyhow::Result<BTreeSet<String>> {
    if git_ref.starts_with('-') {
        bail!("invalid git ref `{git_ref}`");
    }
    let range = format!("{git_ref}...HEAD");
    let mut files = git(
        root,
        &["diff", "--name-only", "-z", "--relative", &range, "--"],
    )?;
    files.extend(git(
        root,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?);
    Ok(files)
}

/// Runs `git <args>` in `root`; returns its NUL-separated stdout entries.
fn git(root: &Path, args: &[&str]) -> anyhow::Result<BTreeSet<String>> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        // Set by git hooks; they would point the diff at another repository.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => {
                anyhow::anyhow!("--changed-since needs git, but `git` was not found on PATH")
            }
            _ => anyhow::Error::new(e).context("cannot run git"),
        })?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Appends the Markdown report to `$GITHUB_STEP_SUMMARY`, when set.
fn append_step_summary(report: &Report, root: &Path, min_severity: Severity) -> anyhow::Result<()> {
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY").filter(|p| !p.is_empty()) else {
        return Ok(());
    };
    let path = PathBuf::from(path);
    let write = || -> io::Result<()> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let mut w = BufWriter::new(file);
        let opts = MarkdownOptions { root, min_severity };
        render_markdown(&mut w, report, &opts)?;
        w.flush()
    };
    write().with_context(|| format!("cannot write step summary {}", path.display()))
}

/// `rules`: every rule (built-in and custom) with its effective severity and state under the config.
fn list_rules(global: &Global, args: &RulesArgs) -> anyhow::Result<ExitCode> {
    let config = Config::load(&global.root, global.config.as_deref())?;
    let statuses: Vec<_> = pulse_engine::rule_statuses(&config)?
        .into_iter()
        .filter(|s| {
            args.category
                .is_none_or(|c| category_of(s.meta.id) == Some(c))
        })
        .collect();
    let mut out = String::new();
    match args.format {
        RulesFormat::Json => {
            out = serde_json::to_string_pretty(&rules_json(&statuses))?;
            out.push('\n');
        }
        RulesFormat::Terminal => rules_table(&mut out, &statuses, global.quiet),
        RulesFormat::Markdown => rules_markdown(&mut out, &statuses),
    }
    print_stdout(&out)?;
    Ok(ExitCode::SUCCESS)
}

/// The `rules` table, plus a count line unless `quiet`.
fn rules_table(out: &mut String, statuses: &[RuleStatus], quiet: bool) {
    use std::fmt::Write as _;
    let id_w = statuses.iter().map(|s| s.meta.id.len()).max().unwrap_or(0);
    let row = |out: &mut String, id: &str, sev: &str, on: &str, name: &str| {
        let _ = writeln!(out, "{id:<id_w$}  {sev:<8}  {on:<7}  {name}");
    };
    row(out, "ID", "SEVERITY", "ENABLED", "NAME");
    for s in statuses {
        let on = if s.enabled { "yes" } else { "no" };
        row(out, s.meta.id, s.severity.as_str(), on, s.meta.name);
    }
    if !quiet {
        let enabled = statuses.iter().filter(|s| s.enabled).count();
        let _ = writeln!(out, "\n{} rules, {enabled} enabled", statuses.len());
    }
}

/// The `rules` table in Markdown, each ID linking to its docs (the README rule table).
fn rules_markdown(out: &mut String, statuses: &[RuleStatus]) {
    use std::fmt::Write as _;
    let cell = |s: &str| s.replace('|', "\\|");
    out.push_str("| Rule | Severity | Name | Summary |\n|---|---|---|---|\n");
    for s in statuses {
        let id = s.meta.id;
        let link = if pulse_rules::docs(id).is_some() {
            format!("[{id}](docs/rules/{id}.md)")
        } else {
            id.to_owned()
        };
        let sev = if s.enabled {
            s.severity.as_str()
        } else {
            "off"
        };
        let _ = writeln!(
            out,
            "| {link} | {sev} | {} | {} |",
            cell(s.meta.name),
            cell(s.meta.summary)
        );
    }
}

fn rules_json(statuses: &[RuleStatus]) -> serde_json::Value {
    statuses
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.meta.id,
                "name": s.meta.name,
                "category": category_of(s.meta.id).map(Category::as_str),
                "severity": s.severity.as_str(),
                "default_severity": s.meta.default_severity.as_str(),
                "enabled": s.enabled,
                "summary": s.meta.summary,
            })
        })
        .collect()
}

/// `explain`: prints the embedded docs of a rule; headings are bold when colored.
fn explain(global: &Global, id: &str) -> anyhow::Result<ExitCode> {
    let id = id.to_ascii_uppercase();
    let Some(doc) = pulse_rules::docs(&id) else {
        let hint = did_you_mean(&id, pulse_rules::all_meta().map(|m| m.id))
            .map_or_else(String::new, |s| format!(" (did you mean `{s}`?)"));
        bail!("unknown rule `{id}`{hint}; run `pulse rules` to list them");
    };
    let color = use_color(global.color, io::stdout().is_terminal(), no_color());
    print_stdout(&if color {
        style_headings(doc)
    } else {
        doc.to_owned()
    })?;
    Ok(ExitCode::SUCCESS)
}

/// Bolds Markdown headings outside code fences.
fn style_headings(doc: &str) -> String {
    let mut out = String::with_capacity(doc.len() + 64);
    let mut fenced = false;
    for line in doc.split_inclusive('\n') {
        if line.starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && line.starts_with('#') {
            let text = line.trim_end_matches(['\n', '\r']);
            out.push_str(&format!("\x1b[1m{text}\x1b[0m"));
            out.push_str(&line[text.len()..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Writes to stdout; a closed pipe (`pulse rules | head`) is not an error.
fn print_stdout(text: &str) -> anyhow::Result<()> {
    let mut w = io::stdout().lock();
    match w.write_all(text.as_bytes()).and_then(|()| w.flush()) {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.context("cannot write to stdout"),
    }
}

/// A `--category` value: a category name or its rule ID prefix, any case.
fn parse_category(s: &str) -> Result<Category, String> {
    [
        Category::Security,
        Category::Structure,
        Category::Efficiency,
        Category::Cognitive,
    ]
    .into_iter()
    .find(|c| s.eq_ignore_ascii_case(c.as_str()) || s.eq_ignore_ascii_case(c.prefix()))
    .ok_or_else(|| format!("unknown category `{s}` (security, structure, efficiency, cognitive)"))
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
        for args in [
            &["init"][..],
            &["scan"],
            &["ci"],
            &["baseline"],
            &["rules"],
            &["explain", "AI-SEC-001"],
        ] {
            let cli =
                Cli::try_parse_from(std::iter::once("pulse").chain(args.iter().copied())).unwrap();
            assert_eq!(cli.command.name(), args[0]);
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
            "--new-only",
            "--changed-since",
            "origin/main",
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
        assert!(args.new_only);
        assert_eq!(args.changed_since.as_deref(), Some("origin/main"));
        assert_eq!(args.format, Some(Format::Github));
    }

    #[test]
    fn parses_rules_flags() {
        let cli =
            Cli::try_parse_from(["pulse", "rules", "--category", "sec", "-f", "json"]).unwrap();
        let Command::Rules(args) = cli.command else {
            panic!("expected rules");
        };
        assert_eq!(args.category, Some(Category::Security));
        assert_eq!(args.format, RulesFormat::Json);
        assert_eq!(parse_category("Cognitive"), Ok(Category::Cognitive));
        assert!(parse_category("style").is_err());
        assert!(Cli::try_parse_from(["pulse", "explain"]).is_err());
    }

    #[test]
    fn style_headings_skips_code_fences() {
        let doc = "# Title\ntext\n```md\n# not a heading\n```\n## Section\n";
        assert_eq!(
            style_headings(doc),
            "\x1b[1m# Title\x1b[0m\ntext\n```md\n# not a heading\n```\n\x1b[1m## Section\x1b[0m\n"
        );
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
