//! High-level runtime facade operations.

use std::sync::Arc;

use susun_engine::{ContainerEngine, ProjectIdentity};
use susun_planner::{DownPlanOptions, ExecutionPlan, UpPlanOptions};
use susun_runtime::{ExecutionReport, Runtime};
use thiserror::Error;

use crate::{AnalysisResult, Planner};

/// Successful runtime operation output.
#[derive(Debug)]
pub struct RuntimeOperationResult {
    /// Immutable plan that was executed.
    pub plan: ExecutionPlan,
    /// Complete execution report.
    pub report: ExecutionReport,
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
    Ok(RuntimeOperationResult { plan, report })
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
    Ok(RuntimeOperationResult { plan, report })
}
