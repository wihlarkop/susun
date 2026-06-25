//! Error type for `ProjectLoader` system-level failures.

use std::path::PathBuf;

use thiserror::Error;

use susun_source::ProviderError;

/// A system-level error that prevents a Compose file from being loaded.
///
/// User mistakes (unknown fields, malformed YAML) are diagnostics in
/// [`LoadResult::report`][crate::loader::LoadResult::report], not this error.
#[derive(Debug, Error)]
pub enum LoadError {
    /// The requested file does not exist.
    #[error("file not found: {}", .path.display())]
    NotFound {
        /// Path that was not found.
        path: PathBuf,
    },
    /// A read or permission error occurred.
    #[error("failed to read {}: {message}", .path.display())]
    Read {
        /// Path that could not be read.
        path: PathBuf,
        /// Human-readable reason.
        message: String,
    },
    /// File exceeds the configured size limit.
    #[error("file too large: {}", .path.display())]
    FileTooLarge {
        /// Path that exceeded the limit.
        path: PathBuf,
    },
    /// File contents are not valid UTF-8.
    #[error("file is not valid UTF-8: {}", .path.display())]
    NotUtf8 {
        /// Path whose encoding is invalid.
        path: PathBuf,
    },
}

impl LoadError {
    /// Converts a [`ProviderError`] into a [`LoadError`] with the given path.
    pub(crate) fn from_provider(path: PathBuf, err: ProviderError) -> Self {
        match err {
            ProviderError::NotFound(_) => LoadError::NotFound { path },
            ProviderError::FileTooLarge { .. } => LoadError::FileTooLarge { path },
            ProviderError::NotUtf8(_) => LoadError::NotUtf8 { path },
            ProviderError::ReadError { message, .. } => LoadError::Read { path, message },
            ProviderError::FileCountExceeded { limit } => LoadError::Read {
                path,
                message: format!("file count limit of {limit} reached"),
            },
        }
    }
}
