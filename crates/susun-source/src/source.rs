//! Source identity types.

use std::path::PathBuf;
use std::sync::Arc;

/// Opaque identifier for a registered source.
///
/// Can only be obtained through [`SourceMap::register`][crate::SourceMap::register].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(pub(crate) u32);

impl SourceId {
    /// Returns the underlying declaration-order index.
    ///
    /// Useful for deterministic ordering (e.g., in diagnostic reports)
    /// without exposing construction.
    pub fn value(self) -> u32 {
        self.0
    }
}

/// Human-readable display name for a source.
///
/// Used in diagnostics (e.g., a file path or `<stdin>`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceName(Arc<str>);

impl SourceName {
    /// Creates a new [`SourceName`].
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self(name.into())
    }
}

impl std::fmt::Display for SourceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for SourceName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A source file or in-memory buffer loaded for parsing.
#[derive(Debug, Clone)]
pub struct LoadedSource {
    /// Human-readable display name for diagnostics.
    pub name: SourceName,
    /// Filesystem path, if this source was read from disk.
    pub path: Option<PathBuf>,
    /// Raw UTF-8 text content.
    pub contents: Arc<str>,
}
