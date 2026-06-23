//! Error type for internal normalization failures.

use thiserror::Error;

/// Error returned by [`normalize`][crate::normalize] for internal invariant violations.
///
/// User-level project mistakes (e.g., unknown service references) are emitted
/// as [`DiagnosticReport`][susun_diagnostics::DiagnosticReport] entries in
/// [`NormalizationOutcome`][crate::normalize::NormalizationOutcome], not as
/// this error type.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NormalizeError {
    /// An internal normalization invariant was violated.
    #[error("internal normalization error: {0}")]
    Internal(String),
}
