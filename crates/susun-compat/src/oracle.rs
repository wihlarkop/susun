//! Versioned oracle configuration for Compose compatibility checks.

use std::path::PathBuf;

use indexmap::IndexMap;

/// Schema version for compatibility configuration files.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct OracleSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl OracleSchemaVersion {
    /// Current schema version emitted by this crate.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

impl Default for OracleSchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

/// Documented Compose implementation used as the compatibility oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ComposeReference {
    /// Human-readable reference name, for example `docker compose`.
    pub name: String,
    /// Version string advertised by the reference implementation.
    pub version: String,
    /// Optional Docker Engine API version paired with this reference.
    pub engine_api_version: Option<String>,
}

/// External command used to invoke the Compose oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct OracleCommand {
    /// Executable name or absolute path.
    pub executable: String,
    /// Arguments placed before operation-specific arguments.
    pub base_args: Vec<String>,
    /// Environment variables passed to the oracle process.
    pub environment: IndexMap<String, String>,
}

impl OracleCommand {
    /// Creates a `docker compose` oracle command.
    #[must_use]
    pub fn docker_compose() -> Self {
        Self {
            executable: "docker".to_owned(),
            base_args: vec!["compose".to_owned()],
            environment: IndexMap::new(),
        }
    }
}

impl Default for OracleCommand {
    fn default() -> Self {
        Self::docker_compose()
    }
}

/// Operation that should be compared against the external Compose oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum OracleOperation {
    /// Compare normalized configuration output.
    Config,
    /// Compare analysis and diagnostics that do not require Docker resources.
    Analyze,
    /// Compare planned runtime actions where Susun can model them.
    Plan,
}

impl OracleOperation {
    /// Returns the Docker Compose subcommand used by the oracle.
    #[must_use]
    pub fn compose_args(self) -> &'static [&'static str] {
        match self {
            Self::Config => &["config", "--format", "json"],
            Self::Analyze => &["config", "--quiet"],
            Self::Plan => &["config", "--format", "json"],
        }
    }
}

/// One compatibility fixture described by the oracle configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FixtureConfig {
    /// Stable fixture identifier used in reports.
    pub id: String,
    /// Directory used as the Compose project root.
    pub project_dir: PathBuf,
    /// Compose files passed to the oracle with repeated `--file` flags.
    pub compose_files: Vec<PathBuf>,
    /// Operations expected for this fixture.
    pub operations: Vec<OracleOperation>,
    /// Optional profile names enabled for this fixture.
    pub profiles: Vec<String>,
    /// Fixture-scoped environment variables.
    pub environment: IndexMap<String, String>,
}

/// Top-level versioned configuration for compatibility oracle runs.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct OracleConfig {
    /// Schema version for this configuration.
    pub schema_version: OracleSchemaVersion,
    /// Compose implementation used as the external oracle.
    pub compose_reference: ComposeReference,
    /// Command used to invoke the external oracle.
    pub command: OracleCommand,
    /// Fixtures included in this compatibility run.
    pub fixtures: Vec<FixtureConfig>,
}
