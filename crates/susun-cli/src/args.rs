//! CLI argument definitions.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

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

    /// Output format for diagnostics.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, global = true)]
    pub format: OutputFormat,

    /// Suppress diagnostic output; exit codes are still preserved.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Color policy for human diagnostics. Currently accepted for CLI compatibility.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, global = true)]
    pub color: ColorChoice,
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
    /// Produce a daemon-free execution plan.
    Plan {
        /// Plan operation.
        #[command(subcommand)]
        command: PlanCommand,
    },
    /// Inspect a previously rendered plan JSON file.
    InspectPlan {
        /// Path to a plan JSON file.
        path: PathBuf,
    },
    /// Bring the project up using Docker Engine.
    Up {
        /// Build service images before starting containers.
        #[arg(long)]
        build: bool,
        /// Run in detached mode. Accepted for Compose compatibility.
        #[arg(long)]
        detach: bool,
        /// Override desired service scale, for example `web=3`.
        #[arg(long = "scale")]
        scale: Vec<String>,
        /// Remove orphan resources where supported.
        #[arg(long)]
        remove_orphans: bool,
        /// Recreate selected service containers even if unchanged.
        #[arg(long)]
        force_recreate: bool,
        /// Refuse container recreation.
        #[arg(long)]
        no_recreate: bool,
        /// Renew anonymous volumes during recreation.
        #[arg(long)]
        renew_anon_volumes: bool,
    },
    /// Build service images.
    Build,
    /// Run a one-off service container.
    Run {
        /// Keep the one-off container after it exits.
        #[arg(long)]
        no_rm: bool,
        /// Service to run.
        service: String,
        /// Command override.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Execute a command in a running service container.
    Exec {
        /// Allocate a pseudo-TTY.
        #[arg(short = 't', long)]
        tty: bool,
        /// Attach stdin.
        #[arg(short = 'i', long)]
        stdin: bool,
        /// Service to exec into.
        service: String,
        /// Command and arguments to execute.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Tear the project down using Docker Engine.
    Down {
        /// Include named volume removal.
        #[arg(long)]
        remove_volumes: bool,
        /// Remove orphan resources where supported.
        #[arg(long)]
        remove_orphans: bool,
    },
    /// List Susun-managed project containers.
    Ps,
    /// Print logs for Susun-managed project containers.
    Logs {
        /// Follow log output.
        #[arg(long)]
        follow: bool,
        /// Include timestamps.
        #[arg(long)]
        timestamps: bool,
        /// Tail line count.
        #[arg(long)]
        tail: Option<usize>,
        /// Selected service names.
        service: Vec<String>,
    },
    /// Start selected project services.
    Start {
        /// Selected service names.
        service: Vec<String>,
    },
    /// Stop selected project services.
    Stop {
        /// Selected service names.
        service: Vec<String>,
    },
    /// Restart selected project services.
    Restart {
        /// Selected service names.
        service: Vec<String>,
    },
}

/// Plan operation subcommands.
#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// Plan service startup.
    Up {
        /// Include build actions for services with build definitions.
        #[arg(long)]
        build: bool,
    },
    /// Plan service teardown.
    Down {
        /// Include named volume removal in the plan.
        #[arg(long)]
        remove_volumes: bool,
    },
}

/// Diagnostic output format.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable diagnostics.
    Human,
    /// Stable JSON diagnostics.
    Json,
}

/// Human output color policy.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ColorChoice {
    /// Auto-detect color support.
    Auto,
    /// Always colorize.
    Always,
    /// Never colorize.
    Never,
}
