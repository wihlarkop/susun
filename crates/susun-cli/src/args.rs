//! CLI argument definitions.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Susun: source-aware Compose file analysis.
#[derive(Debug, Parser)]
#[command(name = "susun", version)]
pub struct Cli {
    #[command(flatten)]
    pub ctx: ContextArgs,

    #[command(subcommand)]
    pub command: Command,
}

/// Context flags that apply to all subcommands (specified before the subcommand name).
#[derive(Debug, Args, Clone)]
pub struct ContextArgs {
    /// Path to the Compose file (repeatable: later files overlay earlier ones).
    ///
    /// When no `-f` flag is given, defaults to `compose.yaml`.
    /// When repeated, files are merged in declaration order.
    #[arg(short = 'f', long = "file", global = true)]
    pub file: Vec<PathBuf>,

    /// Path to a `.env`-format file whose variables override the default `.env`.
    #[arg(long, global = true)]
    pub env_file: Option<PathBuf>,

    /// Override the project name.
    #[arg(short = 'p', long = "project-name", global = true)]
    pub project_name: Option<String>,

    /// Activate a profile. Repeatable (e.g. `--profile debug --profile metrics`).
    #[arg(long, global = true)]
    pub profile: Vec<String>,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check a Compose file for user errors.
    ///
    /// Exits 0 when clean, 1 when user errors are found, 2 on system errors.
    Check,
    /// Emit the resolved project as JSON.
    ///
    /// Prints canonical JSON to stdout on success.
    /// Exits 1 if the file has errors, 2 on system errors.
    Config,
}
