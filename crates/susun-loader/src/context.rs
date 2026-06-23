//! Load context: configuration for a single loader invocation.

use std::path::PathBuf;

/// Configuration for a single [`ProjectLoader`][crate::loader::ProjectLoader] run.
pub struct LoadContext {
    /// Path to the primary Compose file.
    pub path: PathBuf,
}
