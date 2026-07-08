//! Susun: source-aware Compose file analysis.
//!
//! This is the public facade crate. Import [`Analyzer`] to analyze Compose
//! files. Lower-level crates (`susun-loader`, `susun-normalize`, etc.) are
//! implementation details and must not be imported directly by applications.

use serde::{Deserialize, Serialize, de::Error as _};
use thiserror::Error;

pub mod analyzer;
pub mod planning;
pub mod profiles;
pub mod render;
pub mod runtime;
pub mod status;
pub mod workspace;

pub use analyzer::{AnalysisResult, Analyzer};
pub use planning::{
    PlanDiagnosticSummary, PlanOutcomeSummary, PlanOutcomeSummarySchemaVersion, Planner,
    parse_execution_plan_json, parse_plan_outcome_summary_json, render_execution_plan_json,
    render_plan_outcome_summary_json,
};
pub use profiles::{
    EngineConnectionProfileSetSummary, EngineConnectionProfileSetSummarySchemaVersion,
    EngineConnectionProfileSummary, parse_engine_connection_profile_set_json,
    parse_engine_connection_profile_set_summary_json, render_engine_connection_profile_set_json,
    render_engine_connection_profile_set_summary_json,
};
pub use render::{
    DiagnosticLabelSummary, DiagnosticReportSummary, DiagnosticReportSummarySchemaVersion,
    DiagnosticSummary, diagnostic_report_summary, parse_diagnostic_report_summary_json,
    render_diagnostic_report_summary_json, render_diagnostics, render_diagnostics_json,
};
pub use runtime::{
    RuntimeOperationError, RuntimeOperationErrorKind, RuntimeOperationErrorSummary,
    RuntimeOperationErrorSummarySchemaVersion, RuntimeOperationResult,
    RuntimeOperationResultSchemaVersion, RuntimeOperationSummary,
    RuntimeOperationSummarySchemaVersion, down_with_engine, down_with_engine_events,
    parse_execution_report_json, parse_runtime_operation_error_summary_json,
    parse_runtime_operation_result_json, parse_runtime_operation_summary_json,
    render_execution_report_json, render_runtime_operation_error_summary_json,
    render_runtime_operation_result_json, render_runtime_operation_summary_json, up_with_engine,
    up_with_engine_events,
};
pub use status::{
    RuntimeContainerStatusSummary, RuntimeOverview, RuntimeOverviewSchemaVersion,
    RuntimeOverviewStatus, RuntimeResourceCountSummary, RuntimeServiceStatusSummary,
    RuntimeStatusSummary, RuntimeStatusSummarySchemaVersion, parse_runtime_overview_json,
    parse_runtime_status_summary_json, render_runtime_overview_json,
    render_runtime_status_summary_json, runtime_overview, runtime_status_from_snapshot,
};
#[cfg(feature = "watch")]
pub use susun_build::Dockerignore;
pub use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
pub use susun_engine::{
    ClientIdentityFiles, ContainerEngine, ContainerId, ContainerRef, ContainerState,
    CopyFromContainerRequest, CopyToContainerRequest, CreateContainerRequest, EngineArchitecture,
    EngineCapabilities, EngineConnectionDisplayName, EngineConnectionError,
    EngineConnectionProfile, EngineConnectionProfileError, EngineConnectionProfileId,
    EngineConnectionProfileSet, EngineEndpoint, EngineEndpointKind, EngineError, EngineEvent,
    EngineOperatingSystem, EngineProbe, EngineSnapshot, EngineVersion, EventsRequest, ExecRequest,
    HealthState, LogEvent, LogSource, LogsRequest, ObservedContainer, ObservedImageRef, Platform,
    PortRequest, ProjectIdentity, ProjectInstanceId, PruneReport, PruneRequest, PruneScope,
    PublishedPortBinding, RedactedEndpoint, RemoveContainerOptions, ReplicaIndex, ResourceName,
    RuntimeDoctorReport, RuntimeDoctorStatus, ServiceInstanceId, SnapshotCompleteness,
    SnapshotField, StopContainerRequest, SupportLevel, TcpEndpoint, TlsConfiguration,
    WaitContainerRequest, WaitContainerResult,
};
pub use susun_graph::DependencyGraph;
pub use susun_loader::LoadContext;
pub use susun_model::{Command, Project, ProjectName, ServiceName};
pub use susun_normalize::selection::ProjectSelection;
pub use susun_planner::{
    BuildPolicy, DownPlanOptions, ExecutionPlan, PlanError, PlanOutcome, PlannedOperation,
    UpPlanOptions, render_plan_json,
};
pub use susun_runtime::{
    ActionExecutionResult, ActionOutput, ActionStatus, CancellationToken, EventSink,
    ExecutionReport, Runtime, RuntimeError, RuntimeEvent, RuntimeOptions,
};
pub use susun_source::SourceMap;
#[cfg(feature = "watch")]
pub use susun_watch::{
    WatchCancellationToken, WatchError, WatchEvent, WatchEventKind, WatchOptions, WatchResult,
    WatchSession,
};
pub use workspace::{
    ProjectResourceSummary, ProjectSummary, ProjectSummarySchemaVersion, SdkProject,
    ServicePortSummary, ServiceSummary, ServiceVolumeSummary, SusunWorkspace,
    parse_project_summary_json, project_identity, project_identity_from_name,
    render_project_summary_json,
};

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

/// Serializable, display-safe analysis error for UI/API consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisErrorSummary {
    /// Serialized analysis error summary schema version.
    pub schema_version: AnalysisErrorSummarySchemaVersion,
    /// Stable error category.
    pub kind: AnalysisErrorKind,
    /// Display-safe error message.
    pub message: String,
}

/// Serialized analysis error summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisErrorSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl AnalysisErrorSummarySchemaVersion {
    /// Current analysis error summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Stable analysis error category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisErrorKind {
    /// The requested Compose file was not found.
    LoadNotFound,
    /// The requested Compose file could not be read.
    LoadRead,
    /// The requested Compose file exceeded the configured size limit.
    LoadFileTooLarge,
    /// The requested Compose file was not valid UTF-8.
    LoadNotUtf8,
    /// An internal normalization invariant was violated.
    Normalize,
}

impl From<&Error> for AnalysisErrorSummary {
    fn from(error: &Error) -> Self {
        let (kind, message) = match error {
            Error::Load(error) => analysis_load_error_summary(error),
            Error::Normalize(_) => (
                AnalysisErrorKind::Normalize,
                "internal normalization error".to_owned(),
            ),
        };
        Self {
            schema_version: AnalysisErrorSummarySchemaVersion::CURRENT,
            kind,
            message,
        }
    }
}

/// Renders an analysis error summary as pretty JSON using the public SDK schema.
pub fn render_analysis_error_summary_json(
    summary: &AnalysisErrorSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses an analysis error summary from JSON using the public SDK schema.
pub fn parse_analysis_error_summary_json(
    input: &str,
) -> Result<AnalysisErrorSummary, serde_json::Error> {
    let summary: AnalysisErrorSummary = serde_json::from_str(input)?;
    if summary.schema_version != AnalysisErrorSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported analysis error summary schema version {}.{}",
            summary.schema_version.major, summary.schema_version.minor
        )));
    }
    Ok(summary)
}

fn analysis_load_error_summary(error: &susun_loader::LoadError) -> (AnalysisErrorKind, String) {
    match error {
        susun_loader::LoadError::NotFound { .. } => (
            AnalysisErrorKind::LoadNotFound,
            "compose file was not found".to_owned(),
        ),
        susun_loader::LoadError::Read { .. } => (
            AnalysisErrorKind::LoadRead,
            "compose file could not be read".to_owned(),
        ),
        susun_loader::LoadError::FileTooLarge { .. } => (
            AnalysisErrorKind::LoadFileTooLarge,
            "compose file exceeds the configured size limit".to_owned(),
        ),
        susun_loader::LoadError::NotUtf8 { .. } => (
            AnalysisErrorKind::LoadNotUtf8,
            "compose file is not valid UTF-8".to_owned(),
        ),
    }
}
