//! Neutral runtime for executing immutable Susun plans.

pub mod cancel;
pub mod error;
pub mod event;
pub mod report;
pub mod retry;
pub mod runtime;
pub mod validate;

pub use cancel::CancellationToken;
pub use error::{PlanValidationError, RuntimeError};
pub use event::{EventSink, RuntimeEvent};
pub use report::{ActionExecutionResult, ActionOutput, ActionStatus, ExecutionReport};
pub use retry::RetryPolicy;
pub use runtime::{Runtime, RuntimeOptions};
pub use validate::validate_plan_for_execution;
