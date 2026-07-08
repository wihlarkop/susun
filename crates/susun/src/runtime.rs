//! High-level runtime facade operations.

use std::sync::Arc;

use serde::{Deserialize, Serialize, de::Error as _};
use susun_engine::{ContainerEngine, ProjectIdentity};
use susun_planner::{DownPlanOptions, ExecutionPlan, UpPlanOptions};
use susun_runtime::{CancellationToken, EventSink, ExecutionReport, Runtime};
use thiserror::Error;

use crate::{AnalysisResult, Planner, planning::validate_execution_plan_schema};

/// Successful runtime operation output.
#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeOperationResult {
    /// Serialized runtime operation result schema version.
    pub schema_version: RuntimeOperationResultSchemaVersion,
    /// Immutable plan that was executed.
    pub plan: ExecutionPlan,
    /// Complete execution report.
    pub report: ExecutionReport,
}

/// Serialized runtime operation result schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOperationResultSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl RuntimeOperationResultSchemaVersion {
    /// Current runtime operation result schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Serializable runtime operation summary for status/history UIs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOperationSummary {
    /// Serialized runtime operation summary schema version.
    pub schema_version: RuntimeOperationSummarySchemaVersion,
    /// Stable plan ID.
    pub plan_id: String,
    /// Executed operation.
    pub operation: String,
    /// Total planned actions.
    pub planned_actions: usize,
    /// Safe planned actions.
    pub safe_actions: usize,
    /// Caution planned actions.
    pub caution_actions: usize,
    /// Destructive planned actions.
    pub destructive_actions: usize,
    /// Total reported actions.
    pub reported_actions: usize,
    /// Succeeded actions.
    pub succeeded: usize,
    /// Failed actions.
    pub failed: usize,
    /// Skipped actions.
    pub skipped: usize,
    /// Cancelled actions.
    pub cancelled: usize,
}

/// Serialized runtime operation summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOperationSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl RuntimeOperationSummarySchemaVersion {
    /// Current runtime operation summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

impl From<&RuntimeOperationResult> for RuntimeOperationSummary {
    fn from(result: &RuntimeOperationResult) -> Self {
        Self {
            schema_version: RuntimeOperationSummarySchemaVersion::CURRENT,
            plan_id: result.plan.plan_id.as_str().to_owned(),
            operation: result.plan.operation.as_str().to_owned(),
            planned_actions: result.plan.summary.total_actions,
            safe_actions: result.plan.summary.safe_actions,
            caution_actions: result.plan.summary.caution_actions,
            destructive_actions: result.plan.summary.destructive_actions,
            reported_actions: result.report.summary.total_actions,
            succeeded: result.report.summary.succeeded,
            failed: result.report.summary.failed,
            skipped: result.report.summary.skipped,
            cancelled: result.report.summary.cancelled,
        }
    }
}

/// Error returned by high-level runtime operations.
#[derive(Debug, Error)]
pub enum RuntimeOperationError {
    /// Analysis did not produce a project.
    #[error("analysis did not produce a project")]
    MissingProject,
    /// Analysis did not produce a dependency graph.
    #[error("analysis did not produce a dependency graph")]
    MissingGraph,
    /// Analysis did not produce service selection.
    #[error("analysis did not produce service selection")]
    MissingSelection,
    /// Engine operation failed.
    #[error(transparent)]
    Engine(#[from] susun_engine::EngineError),
    /// Planning failed.
    #[error(transparent)]
    Plan(#[from] susun_planner::PlanError),
    /// Planner produced blocking diagnostics.
    #[error("planner diagnostics blocked execution")]
    Blocked,
    /// Runtime failed before a complete report could be produced.
    #[error(transparent)]
    Runtime(#[from] susun_runtime::RuntimeError),
}

/// Plans and executes `up` with a supplied engine.
pub async fn up_with_engine<E>(
    analysis: &AnalysisResult,
    identity: ProjectIdentity,
    engine: Arc<E>,
    options: UpPlanOptions,
) -> Result<RuntimeOperationResult, RuntimeOperationError>
where
    E: ContainerEngine + 'static,
{
    let capabilities = engine.capabilities().await?;
    let snapshot = engine.snapshot(&identity).await?;
    let planner = Planner::new(identity, capabilities, snapshot);
    let outcome = planner.plan_up(analysis, options)?;
    let Some(plan) = outcome.plan else {
        return Err(RuntimeOperationError::Blocked);
    };
    let report = Runtime::new(engine).apply(&plan).await?;
    Ok(RuntimeOperationResult::new(plan, report))
}

/// Plans and executes `down` with a supplied engine.
pub async fn down_with_engine<E>(
    analysis: &AnalysisResult,
    identity: ProjectIdentity,
    engine: Arc<E>,
    options: DownPlanOptions,
) -> Result<RuntimeOperationResult, RuntimeOperationError>
where
    E: ContainerEngine + 'static,
{
    let capabilities = engine.capabilities().await?;
    let snapshot = engine.snapshot(&identity).await?;
    let planner = Planner::new(identity, capabilities, snapshot);
    let outcome = planner.plan_down(analysis, options)?;
    let Some(plan) = outcome.plan else {
        return Err(RuntimeOperationError::Blocked);
    };
    let report = Runtime::new(engine).apply(&plan).await?;
    Ok(RuntimeOperationResult::new(plan, report))
}

/// Plans and executes `up` with a supplied engine, streaming runtime events to
/// `events` and honoring `cancellation`.
pub async fn up_with_engine_events<E>(
    analysis: &AnalysisResult,
    identity: ProjectIdentity,
    engine: Arc<E>,
    options: UpPlanOptions,
    events: EventSink,
    cancellation: CancellationToken,
) -> Result<RuntimeOperationResult, RuntimeOperationError>
where
    E: ContainerEngine + 'static,
{
    let capabilities = engine.capabilities().await?;
    let snapshot = engine.snapshot(&identity).await?;
    let planner = Planner::new(identity, capabilities, snapshot);
    let outcome = planner.plan_up(analysis, options)?;
    let Some(plan) = outcome.plan else {
        return Err(RuntimeOperationError::Blocked);
    };
    let report = Runtime::new(engine)
        .with_events(events)
        .apply_cancellable(&plan, cancellation)
        .await?;
    Ok(RuntimeOperationResult::new(plan, report))
}

/// Plans and executes `down` with a supplied engine, streaming runtime events to
/// `events` and honoring `cancellation`.
pub async fn down_with_engine_events<E>(
    analysis: &AnalysisResult,
    identity: ProjectIdentity,
    engine: Arc<E>,
    options: DownPlanOptions,
    events: EventSink,
    cancellation: CancellationToken,
) -> Result<RuntimeOperationResult, RuntimeOperationError>
where
    E: ContainerEngine + 'static,
{
    let capabilities = engine.capabilities().await?;
    let snapshot = engine.snapshot(&identity).await?;
    let planner = Planner::new(identity, capabilities, snapshot);
    let outcome = planner.plan_down(analysis, options)?;
    let Some(plan) = outcome.plan else {
        return Err(RuntimeOperationError::Blocked);
    };
    let report = Runtime::new(engine)
        .with_events(events)
        .apply_cancellable(&plan, cancellation)
        .await?;
    Ok(RuntimeOperationResult::new(plan, report))
}

impl RuntimeOperationResult {
    fn new(plan: ExecutionPlan, report: ExecutionReport) -> Self {
        Self {
            schema_version: RuntimeOperationResultSchemaVersion::CURRENT,
            plan,
            report,
        }
    }
}

/// Renders an execution report as pretty JSON using the public SDK schema.
pub fn render_execution_report_json(report: &ExecutionReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

/// Parses an execution report from JSON using the public SDK schema.
pub fn parse_execution_report_json(input: &str) -> Result<ExecutionReport, serde_json::Error> {
    let report: ExecutionReport = serde_json::from_str(input)?;
    validate_execution_report_consistency(&report)?;
    Ok(report)
}

/// Renders a runtime operation result as pretty JSON using the public SDK schema.
pub fn render_runtime_operation_result_json(
    result: &RuntimeOperationResult,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(result)
}

/// Parses a runtime operation result from JSON using the public SDK schema.
pub fn parse_runtime_operation_result_json(
    input: &str,
) -> Result<RuntimeOperationResult, serde_json::Error> {
    let result: RuntimeOperationResult = serde_json::from_str(input)?;
    validate_runtime_operation_result(&result)?;
    Ok(result)
}

/// Renders a runtime operation summary as pretty JSON using the public SDK schema.
pub fn render_runtime_operation_summary_json(
    summary: &RuntimeOperationSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a runtime operation summary from JSON using the public SDK schema.
pub fn parse_runtime_operation_summary_json(
    input: &str,
) -> Result<RuntimeOperationSummary, serde_json::Error> {
    let summary: RuntimeOperationSummary = serde_json::from_str(input)?;
    if summary.schema_version != RuntimeOperationSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported runtime operation summary schema version {}.{}",
            summary.schema_version.major, summary.schema_version.minor
        )));
    }
    Ok(summary)
}

fn validate_runtime_operation_result(
    result: &RuntimeOperationResult,
) -> Result<(), serde_json::Error> {
    if result.schema_version != RuntimeOperationResultSchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported runtime operation result schema version {}.{}",
            result.schema_version.major, result.schema_version.minor
        )));
    }
    validate_execution_plan_schema(&result.plan)?;
    validate_execution_report_consistency(&result.report)?;
    if result.report.plan_id != result.plan.plan_id {
        return Err(serde_json::Error::custom(
            "runtime operation report plan_id does not match execution plan",
        ));
    }
    Ok(())
}

fn validate_execution_report_consistency(
    report: &ExecutionReport,
) -> Result<(), serde_json::Error> {
    if report.summary.total_actions != report.actions.len() {
        return Err(serde_json::Error::custom(format!(
            "execution report total_actions {} does not match {} action result(s)",
            report.summary.total_actions,
            report.actions.len()
        )));
    }
    Ok(())
}
