//! High-level planning facade.

use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity};
use susun_planner::{
    DownPlanOptions, NamingPolicy, PlanError, PlanOutcome, PlanningInput, SusunNamingPolicy,
    UpPlanOptions, plan_down, plan_up,
};

use crate::AnalysisResult;

const PHASE_ONE_BLOCKED: &str = "SUS-PLAN-100";

/// High-level planner facade for library consumers.
pub struct Planner {
    identity: ProjectIdentity,
    capabilities: EngineCapabilities,
    snapshot: EngineSnapshot,
    naming: Box<dyn NamingPolicy>,
}

impl Planner {
    /// Creates a planner with default Susun naming.
    pub fn new(
        identity: ProjectIdentity,
        capabilities: EngineCapabilities,
        snapshot: EngineSnapshot,
    ) -> Self {
        Self {
            identity,
            capabilities,
            snapshot,
            naming: Box::new(SusunNamingPolicy::new()),
        }
    }

    /// Replaces the runtime naming policy.
    pub fn with_naming_policy(mut self, naming: impl NamingPolicy + 'static) -> Self {
        self.naming = Box::new(naming);
        self
    }

    /// Plans `up` from a completed Phase 1 analysis result.
    pub fn plan_up(
        &self,
        analysis: &AnalysisResult,
        options: UpPlanOptions,
    ) -> Result<PlanOutcome, PlanError> {
        let Some(input) = self.input_from_analysis(analysis) else {
            return Ok(blocked_by_phase_one());
        };

        plan_up(&input, options, self.naming.as_ref())
    }

    /// Plans `down` from a completed Phase 1 analysis result.
    pub fn plan_down(
        &self,
        analysis: &AnalysisResult,
        options: DownPlanOptions,
    ) -> Result<PlanOutcome, PlanError> {
        let Some(input) = self.input_from_analysis(analysis) else {
            return Ok(blocked_by_phase_one());
        };

        plan_down(&input, options)
    }

    /// Returns the standard blocked outcome for incomplete analysis.
    pub fn blocked_by_analysis() -> PlanOutcome {
        blocked_by_phase_one()
    }

    fn input_from_analysis<'a>(
        &'a self,
        analysis: &'a AnalysisResult,
    ) -> Option<PlanningInput<'a>> {
        Some(PlanningInput {
            project: analysis.project.as_ref()?,
            selection: analysis.selection.as_ref()?,
            graph: analysis.graph.as_ref()?,
            provenance: None,
            identity: &self.identity,
            capabilities: &self.capabilities,
            snapshot: &self.snapshot,
        })
    }
}

fn blocked_by_phase_one() -> PlanOutcome {
    let mut diagnostics = DiagnosticReport::new();
    diagnostics.push(Diagnostic::new(
        PHASE_ONE_BLOCKED,
        Severity::Error,
        "Phase 1 analysis did not produce a project, selection, and dependency graph",
    ));
    PlanOutcome::blocked(diagnostics)
}
