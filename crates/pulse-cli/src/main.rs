//! `pulse`: static analyzer and Quality Gate for AI assets.
#![allow(clippy::print_stdout)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    Init,
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
            Self::Init => "init",
            Self::Scan => "scan",
            Self::Ci => "ci",
            Self::Baseline => "baseline",
            Self::Rules => "rules",
            Self::Explain => "explain",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!("pulse {}: not implemented yet", cli.command.name());
    ExitCode::from(EXIT_USAGE)
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
    fn parses_every_subcommand() {
        for name in ["init", "scan", "ci", "baseline", "rules", "explain"] {
            let cli = Cli::try_parse_from(["pulse", name]).unwrap();
            assert_eq!(cli.command.name(), name);
        }
    }
}
