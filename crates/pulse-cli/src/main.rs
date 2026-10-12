//! `pulse`: static analyzer and Quality Gate for AI assets.
#![allow(clippy::print_stdout)]

use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, bail};
use clap::{Args, Parser, Subcommand};
use pulse_core::DEFAULT_TOML;
use pulse_core::config::CONFIG_FILE;

/// Exit code for usage, config or I/O errors.
const EXIT_USAGE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "pulse",
    version,
    about = "Static analyzer and Quality Gate for AI assets"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a starter pulse.toml
    Init(InitArgs),
    /// Analyze AI assets and print findings
    Scan,
    /// Scan and enforce the Quality Gate
    Ci,
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
            Self::Scan => "scan",
            Self::Ci => "ci",
            Self::Baseline => "baseline",
            Self::Rules => "rules",
            Self::Explain => "explain",
        }
    }
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Project root to write pulse.toml into
    #[arg(short = 'C', long = "root", default_value = ".")]
    root: PathBuf,
    /// Overwrite an existing pulse.toml
    #[arg(long)]
    force: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Init(args) => init(&args.root, args.force).map(|path| {
            println!("created {}", path.display());
        }),
        other => Err(anyhow::anyhow!(
            "pulse {}: not implemented yet",
            other.name()
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(EXIT_USAGE)
        }
    }
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
        assert_eq!(args.root, PathBuf::from("x"));
        assert!(args.force);
    }

    #[test]
    fn parses_every_subcommand() {
        for name in ["init", "scan", "ci", "baseline", "rules", "explain"] {
            let cli = Cli::try_parse_from(["pulse", name]).unwrap();
            assert_eq!(cli.command.name(), name);
        }
    }
}
