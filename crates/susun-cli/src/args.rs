//! CLI argument definitions.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Susun: source-aware Compose file analysis.
#[derive(Debug, Parser)]
#[command(name = "susun", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check a Compose file for user errors.
    ///
    /// Exits 0 when clean, 1 when user errors are found, 2 on system errors.
    Check {
        /// Path to the Compose file to check.
        file: PathBuf,
    },
    /// Emit the resolved project as JSON.
    ///
    /// Prints canonical JSON to stdout on success.
    /// Exits 1 if the file has errors, 2 on system errors.
    Config {
        /// Path to the Compose file to analyse.
        file: PathBuf,
    },
}
