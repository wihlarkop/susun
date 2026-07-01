//! Susun: source-aware Compose file analysis.
//!
//! This is the public facade crate. Import [`Analyzer`] to analyze Compose
//! files. Lower-level crates (`susun-loader`, `susun-normalize`, etc.) are
//! implementation details and must not be imported directly by applications.

pub mod analyzer;
pub mod planning;
pub mod render;
pub mod runtime;
pub mod workspace;

pub use analyzer::{AnalysisResult, Analyzer};
pub use planning::Planner;
pub use render::{render_diagnostics, render_diagnostics_json};
pub use runtime::{
    RuntimeOperationError, RuntimeOperationResult, down_with_engine, up_with_engine,
};
pub use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
pub use susun_engine::{
    EngineCapabilities, EngineSnapshot, ProjectIdentity, ProjectInstanceId, SupportLevel,
};
pub use susun_loader::LoadContext;
pub use susun_model::{Project, ProjectName, ServiceName};
pub use susun_planner::{
    BuildPolicy, DownPlanOptions, ExecutionPlan, PlanError, PlanOutcome, PlannedOperation,
    UpPlanOptions,
};
pub use susun_runtime::{ExecutionReport, RuntimeError};
pub use workspace::{
    ProjectResourceSummary, ProjectSummary, ProjectSummarySchemaVersion, SdkProject,
    ServicePortSummary, ServiceSummary, ServiceVolumeSummary, SusunWorkspace, project_identity,
    project_identity_from_name,
};

use thiserror::Error;

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
