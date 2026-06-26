//! Conservative runtime retry policy.

use susun_engine::{EngineError, EngineOperation};
use susun_planner::PlanAction;

/// Retry policy for action execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maximum attempts for retryable actions.
    pub max_attempts: u32,
}

impl RetryPolicy {
    /// Returns whether an action failure may be retried.
    pub fn should_retry(self, action: &PlanAction, error: &EngineError, attempts: u32) -> bool {
        if attempts >= self.max_attempts {
            return false;
        }
        matches!(action, PlanAction::PullImage(_)) && is_transient_pull_error(error)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 2 }
    }
}

fn is_transient_pull_error(error: &EngineError) -> bool {
    match error {
        EngineError::Connection(_) => true,
        EngineError::Api { operation, .. } => *operation == EngineOperation::PullImage,
        _ => false,
    }
}
