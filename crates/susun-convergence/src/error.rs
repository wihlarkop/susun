//! Convergence error types.

use thiserror::Error;

/// Errors that indicate convergence planner invariants failed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConvergenceError {
    /// Fingerprint generation violated an invariant.
    #[error("fingerprint invariant failed: {detail}")]
    FingerprintInvariant {
        /// Redacted detail.
        detail: String,
    },
    /// Ownership indexing violated an invariant.
    #[error("ownership index invariant failed: {detail}")]
    OwnershipIndexInvariant {
        /// Redacted detail.
        detail: String,
    },
    /// Planner contract was violated.
    #[error("planner contract failed: {detail}")]
    PlannerContract {
        /// Redacted detail.
        detail: String,
    },
}
