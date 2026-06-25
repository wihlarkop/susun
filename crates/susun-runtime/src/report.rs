//! Execution reports.

use std::time::SystemTime;

use indexmap::IndexMap;
use susun_engine::{ContainerRef, EngineImageRef, NetworkRef, VolumeRef};
use susun_planner::{ActionId, ExecutionPlan, PlanId};

/// Runtime action status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ActionStatus {
    /// Action has not started.
    Pending,
    /// Action is ready.
    Ready,
    /// Action is running.
    Running,
    /// Action succeeded.
    Succeeded,
    /// Action failed.
    Failed,
    /// Action skipped due to failed dependency.
    SkippedDependencyFailed,
    /// Action cancelled.
    Cancelled,
}

/// Output produced by a successful action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", content = "payload", rename_all = "snake_case")
)]
pub enum ActionOutput {
    /// Pulled image.
    Image(EngineImageRef),
    /// Created container.
    Container(ContainerRef),
    /// Created network.
    Network(NetworkRef),
    /// Created volume.
    Volume(VolumeRef),
    /// No output.
    None,
}

/// Per-action result.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionExecutionResult {
    /// Action ID.
    pub action_id: ActionId,
    /// Final status.
    pub status: ActionStatus,
    /// Start timestamp.
    pub started_at: Option<SystemTime>,
    /// Finish timestamp.
    pub finished_at: Option<SystemTime>,
    /// Attempt count.
    pub attempts: u32,
    /// Successful output.
    pub output: Option<ActionOutput>,
    /// Redacted error message.
    pub error: Option<String>,
}

impl ActionExecutionResult {
    /// Creates a pending result.
    pub fn pending(action_id: ActionId) -> Self {
        Self {
            action_id,
            status: ActionStatus::Pending,
            started_at: None,
            finished_at: None,
            attempts: 0,
            output: None,
            error: None,
        }
    }
}

/// Execution summary.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecutionSummary {
    /// Total actions.
    pub total_actions: usize,
    /// Succeeded actions.
    pub succeeded: usize,
    /// Failed actions.
    pub failed: usize,
    /// Skipped actions.
    pub skipped: usize,
    /// Cancelled actions.
    pub cancelled: usize,
}

/// Complete execution report.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecutionReport {
    /// Plan ID.
    pub plan_id: PlanId,
    /// Action results keyed by action ID.
    pub actions: IndexMap<ActionId, ActionExecutionResult>,
    /// Summary.
    pub summary: ExecutionSummary,
}

impl ExecutionReport {
    /// Creates a report with every action pending.
    pub fn pending(plan: &ExecutionPlan) -> Self {
        let actions = plan
            .actions
            .keys()
            .map(|id| (id.clone(), ActionExecutionResult::pending(id.clone())))
            .collect();
        Self {
            plan_id: plan.plan_id.clone(),
            actions,
            summary: ExecutionSummary {
                total_actions: plan.actions.len(),
                ..ExecutionSummary::default()
            },
        }
    }

    /// Recomputes summary counters.
    pub fn refresh_summary(&mut self) {
        let mut summary = ExecutionSummary {
            total_actions: self.actions.len(),
            ..ExecutionSummary::default()
        };
        for result in self.actions.values() {
            match result.status {
                ActionStatus::Succeeded => summary.succeeded += 1,
                ActionStatus::Failed => summary.failed += 1,
                ActionStatus::SkippedDependencyFailed => summary.skipped += 1,
                ActionStatus::Cancelled => summary.cancelled += 1,
                ActionStatus::Pending | ActionStatus::Ready | ActionStatus::Running => {}
            }
        }
        self.summary = summary;
    }
}
