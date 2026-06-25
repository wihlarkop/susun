//! Source providers: filesystem and in-memory implementations.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use thiserror::Error;

use crate::source::{LoadedSource, SourceName};

/// A request to load a source file.
#[derive(Debug, Clone)]
pub struct SourceRequest {
    /// Path of the file to load.
    pub path: PathBuf,
    /// Optional override for the display name; defaults to the path string.
    pub display_name: Option<SourceName>,
}

impl SourceRequest {
    /// Creates a request for the given path with no display name override.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            display_name: None,
        }
    }
}

/// Limits applied by source providers to prevent unbounded reads.
#[derive(Debug, Clone, Copy)]
pub struct LoadLimits {
    /// Maximum file size in bytes.
    pub max_file_bytes: u64,
    /// Maximum number of files that may be loaded by one provider instance.
    pub max_file_count: usize,
}

impl LoadLimits {
    /// Sensible defaults: 10 MiB per file, 1 000 files per session.
    pub const DEFAULT: Self = Self {
        max_file_bytes: 10 * 1024 * 1024,
        max_file_count: 1_000,
    };
}

impl Default for LoadLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Errors from provider operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    /// The requested file does not exist.
    #[error("file not found: {0}")]
    NotFound(PathBuf),
    /// A read or permission error occurred.
    #[error("could not read {path}: {message}")]
    ReadError {
        /// The path that could not be read.
        path: PathBuf,
        /// Human-readable reason.
        message: String,
    },
    /// File exceeds the configured size limit.
    #[error("file {path} is {actual_bytes} bytes, exceeds limit of {limit_bytes}")]
    FileTooLarge {
        /// The path that was too large.
        path: PathBuf,
        /// Actual file size in bytes.
        actual_bytes: u64,
        /// Configured maximum in bytes.
        limit_bytes: u64,
    },
    /// Provider has already loaded the maximum number of files.
    #[error("file count limit of {limit} reached")]
    FileCountExceeded {
        /// The configured limit.
        limit: usize,
    },
    /// File contents are not valid UTF-8.
    #[error("file {0} is not valid UTF-8")]
    NotUtf8(PathBuf),
}

/// Loads source text for a given request.
///
/// Implementations must not allocate [`SourceId`][crate::SourceId]s;
/// that is the exclusive responsibility of [`SourceMap`][crate::SourceMap].
pub trait SourceProvider: Send + Sync {
    /// Reads and returns the source identified by `request`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] on I/O errors, missing files, or limit violations.
    fn read(&self, request: &SourceRequest) -> Result<LoadedSource, ProviderError>;
}

/// Reads files from the real filesystem, enforcing [`LoadLimits`].
#[derive(Debug)]
pub struct FileSystemSourceProvider {
    limits: LoadLimits,
    count: AtomicUsize,
}

impl FileSystemSourceProvider {
    /// Creates a new provider with the given limits.
    pub fn new(limits: LoadLimits) -> Self {
        Self {
            limits,
            count: AtomicUsize::new(0),
        }
    }

    /// Creates a provider with [`LoadLimits::DEFAULT`].
    pub fn with_default_limits() -> Self {
        Self::new(LoadLimits::DEFAULT)
    }
}

impl SourceProvider for FileSystemSourceProvider {
    fn read(&self, request: &SourceRequest) -> Result<LoadedSource, ProviderError> {
        let prev = self.count.fetch_add(1, Ordering::SeqCst);
        if prev >= self.limits.max_file_count {
            self.count.fetch_sub(1, Ordering::SeqCst);
            return Err(ProviderError::FileCountExceeded {
                limit: self.limits.max_file_count,
            });
        }

        let meta = std::fs::metadata(&request.path).map_err(|e| {
            self.count.fetch_sub(1, Ordering::SeqCst);
            if e.kind() == std::io::ErrorKind::NotFound {
                ProviderError::NotFound(request.path.clone())
            } else {
                ProviderError::ReadError {
                    path: request.path.clone(),
                    message: e.to_string(),
                }
            }
        })?;

        if meta.len() > self.limits.max_file_bytes {
            self.count.fetch_sub(1, Ordering::SeqCst);
            return Err(ProviderError::FileTooLarge {
                path: request.path.clone(),
                actual_bytes: meta.len(),
                limit_bytes: self.limits.max_file_bytes,
            });
        }

        let raw = std::fs::read(&request.path).map_err(|e| {
            self.count.fetch_sub(1, Ordering::SeqCst);
            ProviderError::ReadError {
                path: request.path.clone(),
                message: e.to_string(),
            }
        })?;

        let text = String::from_utf8(raw).map_err(|_| {
            self.count.fetch_sub(1, Ordering::SeqCst);
            ProviderError::NotUtf8(request.path.clone())
        })?;

        let name = request
            .display_name
            .clone()
            .unwrap_or_else(|| SourceName::new(request.path.to_string_lossy().as_ref()));

        Ok(LoadedSource {
            name,
            path: Some(request.path.clone()),
            contents: Arc::from(text.as_str()),
        })
    }
}

/// In-memory source provider for testing and sandboxed execution.
///
/// Never allocates [`SourceId`][crate::SourceId]s — that remains the
/// responsibility of [`SourceMap`][crate::SourceMap].
#[derive(Debug)]
pub struct MemorySourceProvider {
    files: HashMap<PathBuf, Arc<str>>,
    limits: LoadLimits,
    count: AtomicUsize,
}

impl MemorySourceProvider {
    /// Creates a provider pre-loaded with the given file map and limits.
    pub fn new(files: HashMap<PathBuf, Arc<str>>, limits: LoadLimits) -> Self {
        Self {
            files,
            limits,
            count: AtomicUsize::new(0),
        }
    }

    /// Creates a provider from an iterator of `(path, contents)` pairs with default limits.
    pub fn with_files<I, P, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = (P, S)>,
        P: Into<PathBuf>,
        S: Into<Arc<str>>,
    {
        let files = iter
            .into_iter()
            .map(|(p, s)| (p.into(), s.into()))
            .collect();
        Self::new(files, LoadLimits::DEFAULT)
    }
}

impl SourceProvider for MemorySourceProvider {
    fn read(&self, request: &SourceRequest) -> Result<LoadedSource, ProviderError> {
        let prev = self.count.fetch_add(1, Ordering::SeqCst);
        if prev >= self.limits.max_file_count {
            self.count.fetch_sub(1, Ordering::SeqCst);
            return Err(ProviderError::FileCountExceeded {
                limit: self.limits.max_file_count,
            });
        }

        let contents = self
            .files
            .get(&request.path)
            .ok_or_else(|| {
                self.count.fetch_sub(1, Ordering::SeqCst);
                ProviderError::NotFound(request.path.clone())
            })?
            .clone();

        let byte_len = contents.len() as u64;
        if byte_len > self.limits.max_file_bytes {
            self.count.fetch_sub(1, Ordering::SeqCst);
            return Err(ProviderError::FileTooLarge {
                path: request.path.clone(),
                actual_bytes: byte_len,
                limit_bytes: self.limits.max_file_bytes,
            });
        }

        let name = request
            .display_name
            .clone()
            .unwrap_or_else(|| SourceName::new(request.path.to_string_lossy().as_ref()));

        Ok(LoadedSource {
            name,
            path: Some(request.path.clone()),
            contents,
        })
    }
}
