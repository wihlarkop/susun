//! Planner errors for internal invariant failures.

use thiserror::Error;

use crate::ActionId;

/// Planner invariant error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanError {
    /// Internal planner invariant was violated.
    #[error("planner invariant violated: {detail}")]
    InvariantViolation {
        /// Invariant failure detail.
        detail: String,
    },
    /// A generated action ID collided with an existing action.
    #[error("action id collision: {id}")]
    ActionIdCollision {
        /// Colliding action ID.
        id: ActionId,
    },
    /// An action references a dependency that is not present in the plan.
    #[error("action {action} references missing dependency {dependency}")]
    InvalidDependencyReference {
        /// Action with invalid dependency.
        action: ActionId,
        /// Missing dependency ID.
        dependency: ActionId,
    },
}
