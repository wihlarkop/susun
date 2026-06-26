//! Build error model.

use std::path::PathBuf;

use thiserror::Error;

/// Build operation that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOperation {
    /// Capability discovery.
    Capabilities,
    /// Build execution.
    Build,
}

/// Build failure.
#[derive(Debug, Error)]
pub enum BuildError {
    /// Build was cancelled before completion.
    #[error("build operation cancelled")]
    Cancelled,
    /// Required capability is unavailable.
    #[error("build capability `{capability}` is not supported")]
    UnsupportedCapability {
        /// Missing capability.
        capability: &'static str,
    },
    /// Build input is invalid.
    #[error("invalid build input: {detail}")]
    InvalidInput {
        /// Redacted detail.
        detail: String,
    },
    /// Build process could not be launched.
    #[error("failed to launch build process `{program}`: {source}")]
    Launch {
        /// Program path or name.
        program: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Build process failed.
    #[error("build process exited with status {status}")]
    ProcessFailed {
        /// Exit status text.
        status: String,
    },
    /// Build output did not include a usable image identity.
    #[error("build completed without an image identity")]
    MissingImageIdentity,
}
