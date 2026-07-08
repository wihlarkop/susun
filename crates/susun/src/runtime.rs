//! High-level runtime facade operations.

use std::sync::Arc;

use serde::{Deserialize, Serialize, de::Error as _};
use susun_engine::{ContainerEngine, EngineConnectionError, EngineError, ProjectIdentity};
use susun_planner::{DownPlanOptions, ExecutionPlan, UpPlanOptions};
use susun_runtime::{CancellationToken, EventSink, ExecutionReport, Runtime, RuntimeError};
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

/// Serializable, redacted runtime operation error for UI/API consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOperationErrorSummary {
    /// Serialized runtime operation error summary schema version.
    pub schema_version: RuntimeOperationErrorSummarySchemaVersion,
    /// Stable error category.
    pub kind: RuntimeOperationErrorKind,
    /// Display-safe error message.
    pub message: String,
}

/// Serialized runtime operation error summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOperationErrorSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl RuntimeOperationErrorSummarySchemaVersion {
    /// Current runtime operation error summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Stable runtime operation error category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationErrorKind {
    /// Analysis did not produce a project.
    MissingProject,
    /// Analysis did not produce a dependency graph.
    MissingGraph,
    /// Analysis did not produce service selection.
    MissingSelection,
    /// Engine interaction failed.
    Engine,
    /// Planning failed before execution.
    Plan,
    /// Planner emitted blocking diagnostics.
    Blocked,
    /// Runtime execution failed before a complete report was produced.
    Runtime,
}

impl From<&RuntimeOperationError> for RuntimeOperationErrorSummary {
    fn from(error: &RuntimeOperationError) -> Self {
        let (kind, message) = match error {
            RuntimeOperationError::MissingProject => (
                RuntimeOperationErrorKind::MissingProject,
                "analysis did not produce a project".to_owned(),
            ),
            RuntimeOperationError::MissingGraph => (
                RuntimeOperationErrorKind::MissingGraph,
                "analysis did not produce a dependency graph".to_owned(),
            ),
            RuntimeOperationError::MissingSelection => (
                RuntimeOperationErrorKind::MissingSelection,
                "analysis did not produce service selection".to_owned(),
            ),
            RuntimeOperationError::Engine(error) => (
                RuntimeOperationErrorKind::Engine,
                engine_error_message(error),
            ),
            RuntimeOperationError::Plan(error) => {
                (RuntimeOperationErrorKind::Plan, error.to_string())
            }
            RuntimeOperationError::Blocked => (
                RuntimeOperationErrorKind::Blocked,
                "planner diagnostics blocked execution".to_owned(),
            ),
            RuntimeOperationError::Runtime(error) => (
                RuntimeOperationErrorKind::Runtime,
                runtime_error_message(error),
            ),
        };
        Self {
            schema_version: RuntimeOperationErrorSummarySchemaVersion::CURRENT,
            kind,
            message,
        }
    }
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

/// Renders a runtime operation error summary as pretty JSON using the public SDK schema.
pub fn render_runtime_operation_error_summary_json(
    summary: &RuntimeOperationErrorSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a runtime operation error summary from JSON using the public SDK schema.
pub fn parse_runtime_operation_error_summary_json(
    input: &str,
) -> Result<RuntimeOperationErrorSummary, serde_json::Error> {
    let summary: RuntimeOperationErrorSummary = serde_json::from_str(input)?;
    if summary.schema_version != RuntimeOperationErrorSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported runtime operation error summary schema version {}.{}",
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

fn engine_error_message(error: &EngineError) -> String {
    match error {
        EngineError::Connection(error) => engine_connection_error_message(error),
        EngineError::Api { operation, .. } => format!("engine {operation} failed"),
        EngineError::Unsupported { capability } => format!("engine does not support {capability}"),
        EngineError::Conflict { resource, .. } => {
            format!("engine resource conflict for {resource}")
        }
        EngineError::NotFound { resource } => format!("engine resource not found: {resource}"),
        EngineError::Authentication { registry } => {
            format!("engine authentication failed for {registry}")
        }
        EngineError::Cancelled => "engine operation cancelled".to_owned(),
    }
}

fn engine_connection_error_message(error: &EngineConnectionError) -> String {
    match error {
        EngineConnectionError::InvalidEndpoint { detail }
        | EngineConnectionError::TlsConfiguration { detail } => detail.clone(),
        EngineConnectionError::UnsupportedEndpoint { .. } => {
            "engine endpoint kind is not supported on this platform".to_owned()
        }
        EngineConnectionError::EndpointUnavailable { .. } => {
            "engine endpoint is unavailable".to_owned()
        }
        EngineConnectionError::ApiNegotiation { .. } => {
            "failed to probe engine API version".to_owned()
        }
        EngineConnectionError::Authentication { .. } => {
            "engine endpoint authentication failed".to_owned()
        }
    }
}

fn runtime_error_message(error: &RuntimeError) -> String {
    match error {
        RuntimeError::InvalidPlan(error) => error.to_string(),
        RuntimeError::Capabilities(_) => "engine capability discovery failed".to_owned(),
        RuntimeError::InternalInvariant { detail } => {
            format!("runtime invariant failed: {detail}")
        }
    }
}
