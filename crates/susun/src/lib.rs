//! Susun: source-aware Compose file analysis.
//!
//! This is the public facade crate. Import [`Analyzer`] to analyze Compose
//! files. Lower-level crates (`susun-loader`, `susun-normalize`, etc.) are
//! implementation details and must not be imported directly by applications.

pub mod analyzer;
pub mod render;

pub use analyzer::{AnalysisResult, Analyzer};
pub use render::render_diagnostics;

use thiserror::Error;

/// Top-level error returned by [`Analyzer::analyze`].
///
/// This represents system-level failures only. User-visible issues (unknown
/// fields, malformed YAML keys) appear as diagnostics in
/// [`AnalysisResult::report`], not as `Err` variants here.
#[derive(Debug, Error)]
pub enum Error {
    /// A system-level error prevented loading the Compose file.
    #[error(transparent)]
    Load(#[from] susun_loader::LoadError),
    /// An internal normalization invariant was violated.
    #[error(transparent)]
    Normalize(#[from] susun_normalize::error::NormalizeError),
}
