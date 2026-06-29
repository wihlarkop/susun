//! Error types for compatibility harness preparation.

use std::path::PathBuf;

use thiserror::Error;

/// Operational errors produced while preparing compatibility runs.
#[derive(Debug, Error)]
pub enum CompatibilityError {
    /// A fixture identifier was empty or only whitespace.
    #[error("compatibility fixture id must not be empty")]
    EmptyFixtureId,

    /// A fixture has no Compose files to compare.
    #[error("compatibility fixture `{fixture_id}` must include at least one compose file")]
    MissingComposeFiles {
        /// Identifier of the invalid fixture.
        fixture_id: String,
    },

    /// A fixture path is not relative to its configured project directory.
    #[error("compatibility fixture `{fixture_id}` uses an absolute compose path: {path}")]
    AbsoluteComposePath {
        /// Identifier of the invalid fixture.
        fixture_id: String,
        /// Compose file path that violated the policy.
        path: PathBuf,
    },

    /// The external oracle command has no executable.
    #[error("compose oracle executable must not be empty")]
    EmptyOracleExecutable,

    /// A corpus manifest has no fixtures.
    #[error("compatibility corpus must include at least one fixture")]
    EmptyCorpus,

    /// A corpus manifest could not be parsed from JSON.
    #[cfg(feature = "serde")]
    #[error("failed to parse compatibility corpus manifest: {0}")]
    CorpusJson(#[from] serde_json::Error),
}
