//! Dockerfile Level A validation.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Validated Dockerfile source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerfileSource {
    /// Canonical Dockerfile path.
    pub path: PathBuf,
    /// Optional selected target stage.
    pub target: Option<String>,
}

/// Dockerfile source validation error.
#[derive(Debug, Error)]
pub enum DockerfileValidationError {
    /// Dockerfile path is missing.
    #[error("dockerfile `{path}` does not exist")]
    Missing {
        /// Dockerfile path.
        path: PathBuf,
    },
    /// Dockerfile source is not a regular file.
    #[error("dockerfile `{path}` is not a regular file")]
    NotFile {
        /// Dockerfile path.
        path: PathBuf,
    },
    /// Selected target uses invalid syntax.
    #[error("dockerfile target `{target}` is not a valid stage name")]
    InvalidTarget {
        /// Invalid target.
        target: String,
    },
    /// Metadata lookup failed.
    #[error("failed to read dockerfile metadata for `{path}`: {source}")]
    Metadata {
        /// Dockerfile path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

/// Validates Level A Dockerfile inputs.
pub fn validate_dockerfile_source(
    dockerfile: &Path,
    target: Option<&str>,
) -> Result<DockerfileSource, DockerfileValidationError> {
    let metadata =
        std::fs::metadata(dockerfile).map_err(|source| DockerfileValidationError::Metadata {
            path: dockerfile.to_path_buf(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(DockerfileValidationError::NotFile {
            path: dockerfile.to_path_buf(),
        });
    }
    let target = target.map(ToOwned::to_owned);
    if let Some(target) = &target {
        if !is_valid_target_name(target) {
            return Err(DockerfileValidationError::InvalidTarget {
                target: target.clone(),
            });
        }
    }
    Ok(DockerfileSource {
        path: dockerfile.to_path_buf(),
        target,
    })
}

fn is_valid_target_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}
