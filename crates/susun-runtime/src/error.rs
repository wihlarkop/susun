//! Runtime errors.

use susun_engine::EngineError;
use susun_planner::{ActionId, PlanError, PlanId};

/// Plan validation failures.
#[derive(Debug, thiserror::Error)]
pub enum PlanValidationError {
    /// Unsupported plan schema.
    #[error("unsupported plan schema {major}.{minor}")]
    UnsupportedSchema {
        /// Major schema version.
        major: u16,
        /// Minor schema version.
        minor: u16,
    },
    /// Action graph is invalid.
    #[error(transparent)]
    InvalidDag(#[from] PlanError),
    /// Plan has no actions.
    #[error("plan {plan_id} has no executable actions")]
    EmptyPlan {
        /// Plan ID.
        plan_id: PlanId,
    },
    /// Action cannot be executed by Phase 3 runtime.
    #[error("action {action_id} is not executable: {reason}")]
    UnsupportedAction {
        /// Action ID.
        action_id: ActionId,
        /// Reason.
        reason: String,
    },
}

/// Runtime-level failures that prevent a meaningful execution report.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Invalid plan.
    #[error(transparent)]
    InvalidPlan(#[from] PlanValidationError),
    /// Capability discovery failed.
    #[error("engine capability discovery failed: {0}")]
    Capabilities(#[source] EngineError),
    /// Runtime invariant failed.
    #[error("runtime invariant failed: {detail}")]
    InternalInvariant {
        /// Detail.
        detail: String,
    },
}
