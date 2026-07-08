//! High-level planning facade.

use serde::{Deserialize, Serialize, de::Error as _};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity};
use susun_planner::{
    DownPlanOptions, ExecutionPlan, NamingPolicy, PlanError, PlanOutcome, PlanningInput,
    SusunNamingPolicy, UpPlanOptions, plan_down, plan_up, render_plan_json,
};

use crate::AnalysisResult;

const PHASE_ONE_BLOCKED: &str = "SUS-PLAN-100";

/// Serializable plan-outcome summary for approval UIs and SDK persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanOutcomeSummary {
    /// Serialized plan outcome summary schema version.
    pub schema_version: PlanOutcomeSummarySchemaVersion,
    /// Whether planning produced an executable plan.
    pub planned: bool,
    /// Stable plan ID when a plan exists.
    pub plan_id: Option<String>,
    /// Planned operation when a plan exists.
    pub operation: Option<String>,
    /// Total action count when a plan exists.
    pub action_count: usize,
    /// Safe action count when a plan exists.
    pub safe_actions: usize,
    /// Caution action count when a plan exists.
    pub caution_actions: usize,
    /// Destructive action count when a plan exists.
    pub destructive_actions: usize,
    /// Whether planning emitted error diagnostics.
    pub has_errors: bool,
    /// Number of planning diagnostics.
    pub diagnostic_count: usize,
    /// Sorted planning diagnostics without source spans.
    pub diagnostics: Vec<PlanDiagnosticSummary>,
}

/// Serialized plan outcome summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanOutcomeSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl PlanOutcomeSummarySchemaVersion {
    /// Current plan outcome summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Serializable planning diagnostic summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanDiagnosticSummary {
    /// Machine-readable diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: String,
    /// Human-readable diagnostic message.
    pub message: String,
}

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

impl From<&PlanOutcome> for PlanOutcomeSummary {
    fn from(outcome: &PlanOutcome) -> Self {
        let diagnostics = outcome
            .diagnostics
            .sorted()
            .into_iter()
            .map(|diagnostic| PlanDiagnosticSummary {
                code: diagnostic.code.as_str().to_owned(),
                severity: diagnostic.severity.to_string(),
                message: diagnostic.message.clone(),
            })
            .collect::<Vec<_>>();
        let plan = outcome.plan.as_ref();
        let summary = plan.map(|plan| &plan.summary);

        Self {
            schema_version: PlanOutcomeSummarySchemaVersion::CURRENT,
            planned: plan.is_some(),
            plan_id: plan.map(|plan| plan.plan_id.as_str().to_owned()),
            operation: plan.map(|plan| plan.operation.as_str().to_owned()),
            action_count: summary.map_or(0, |summary| summary.total_actions),
            safe_actions: summary.map_or(0, |summary| summary.safe_actions),
            caution_actions: summary.map_or(0, |summary| summary.caution_actions),
            destructive_actions: summary.map_or(0, |summary| summary.destructive_actions),
            has_errors: outcome.diagnostics.has_errors(),
            diagnostic_count: diagnostics.len(),
            diagnostics,
        }
    }
}

/// Renders an execution plan as pretty JSON using the public SDK schema.
pub fn render_execution_plan_json(plan: &ExecutionPlan) -> Result<String, serde_json::Error> {
    render_plan_json(plan)
}

/// Parses an execution plan from JSON using the public SDK schema.
pub fn parse_execution_plan_json(input: &str) -> Result<ExecutionPlan, serde_json::Error> {
    serde_json::from_str(input)
}

/// Renders a plan outcome summary as pretty JSON using the public SDK schema.
pub fn render_plan_outcome_summary_json(
    summary: &PlanOutcomeSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a plan outcome summary from JSON using the public SDK schema.
pub fn parse_plan_outcome_summary_json(
    input: &str,
) -> Result<PlanOutcomeSummary, serde_json::Error> {
    let summary: PlanOutcomeSummary = serde_json::from_str(input)?;
    if summary.schema_version != PlanOutcomeSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported plan outcome summary schema version {}.{}",
            summary.schema_version.major, summary.schema_version.minor
        )));
    }
    Ok(summary)
}
