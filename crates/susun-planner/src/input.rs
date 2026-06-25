//! Explicit planner inputs and outcomes.

use susun_diagnostics::DiagnosticReport;
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity};
use susun_graph::DependencyGraph;
use susun_model::Project;
use susun_normalize::{provenance::ProjectProvenance, selection::ProjectSelection};

use crate::ExecutionPlan;

/// Explicit input bundle for pure planning.
pub struct PlanningInput<'a> {
    /// Canonical project.
    pub project: &'a Project,
    /// Active service selection.
    pub selection: &'a ProjectSelection,
    /// Dependency graph for active services.
    pub graph: &'a DependencyGraph,
    /// Optional source provenance for diagnostics.
    pub provenance: Option<&'a ProjectProvenance>,
    /// Stable project identity.
    pub identity: &'a ProjectIdentity,
    /// Explicit engine capabilities.
    pub capabilities: &'a EngineCapabilities,
    /// Explicit engine snapshot.
    pub snapshot: &'a EngineSnapshot,
}

/// Planner outcome with diagnostics.
#[derive(Debug)]
pub struct PlanOutcome {
    /// Plan when planning succeeded.
    pub plan: Option<ExecutionPlan>,
    /// Diagnostics emitted during planning.
    pub diagnostics: DiagnosticReport,
}

impl PlanOutcome {
    /// Creates an outcome from a successful plan.
    pub fn planned(plan: ExecutionPlan, diagnostics: DiagnosticReport) -> Self {
        Self {
            plan: Some(plan),
            diagnostics,
        }
    }

    /// Creates an outcome blocked by diagnostics.
    pub fn blocked(diagnostics: DiagnosticReport) -> Self {
        Self {
            plan: None,
            diagnostics,
        }
    }
}
