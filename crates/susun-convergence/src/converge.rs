//! Pure convergence planning entry point.

use indexmap::IndexMap;
use susun_diagnostics::DiagnosticReport;
use susun_engine::{EngineCapabilities, ServiceInstanceId};
use susun_planner::{ExecutionPlan, PlannedOperation};

use crate::{
    ConvergenceDecision, ConvergenceDecisionKind, ConvergenceError, ConvergencePlanFragment,
    ConvergencePolicy, DesiredDeployment, DesiredInstanceFingerprints, InstanceDifference,
    ObservedDeployment, classify_deployment_differences, classify_scale_delta,
    convergence_diagnostic_for_difference, plan_noop_or_start, plan_scale,
};

/// Pure convergence planner input.
pub struct ConvergenceInput<'a> {
    /// Desired deployment.
    pub desired: &'a DesiredDeployment,
    /// Observed deployment.
    pub observed: &'a ObservedDeployment,
    /// Engine capabilities.
    pub capabilities: &'a EngineCapabilities,
    /// Convergence policy.
    pub policy: &'a ConvergencePolicy,
    /// Desired fingerprints by instance.
    pub desired_fingerprints: &'a DesiredInstanceFingerprints,
}

/// Pure convergence planner output.
pub struct ConvergenceOutcome {
    /// Decisions by service instance.
    pub decisions: IndexMap<ServiceInstanceId, ConvergenceDecision>,
    /// Optional execution plan.
    pub plan: Option<ExecutionPlan>,
    /// Diagnostics emitted while planning convergence.
    pub diagnostics: DiagnosticReport,
}

/// Plans convergence without performing daemon operations.
pub fn plan_convergence(
    input: ConvergenceInput<'_>,
) -> Result<ConvergenceOutcome, ConvergenceError> {
    let differences =
        classify_deployment_differences(input.desired, input.observed, input.desired_fingerprints);
    let mut decisions = IndexMap::new();
    let mut diagnostics = DiagnosticReport::new();
    let mut fragment = ConvergencePlanFragment::new();

    for (instance, difference) in differences {
        if let Some(diagnostic) = convergence_diagnostic_for_difference(&instance, &difference) {
            diagnostics.push(diagnostic);
        }
        if let Some(node) = plan_noop_or_start(&instance, &difference) {
            fragment.insert(node);
        }
        let kind = decision_kind(&difference);
        decisions.insert(
            instance.clone(),
            ConvergenceDecision::new(instance, difference, kind),
        );
    }

    let scale_fragment = plan_scale(&classify_scale_delta(input.desired, input.observed));
    for node in scale_fragment.actions.into_values() {
        fragment.insert(node);
    }

    let plan = if diagnostics.has_errors() {
        None
    } else {
        Some(ExecutionPlan::new(
            input.desired.identity.clone(),
            PlannedOperation::Up,
            fragment.actions,
            DiagnosticReport::new(),
        ))
    };

    Ok(ConvergenceOutcome {
        decisions,
        plan,
        diagnostics,
    })
}

fn decision_kind(difference: &InstanceDifference) -> ConvergenceDecisionKind {
    match difference {
        InstanceDifference::Missing => ConvergenceDecisionKind::Create,
        InstanceDifference::Unchanged => ConvergenceDecisionKind::NoOp,
        InstanceDifference::StoppedButCompatible => ConvergenceDecisionKind::Start,
        InstanceDifference::ImageChanged | InstanceDifference::ConfigurationChanged { .. } => {
            ConvergenceDecisionKind::Recreate
        }
        InstanceDifference::RuntimeStateDrift { .. } => ConvergenceDecisionKind::Restart,
        InstanceDifference::OwnershipAmbiguous | InstanceDifference::UnsupportedObservedState => {
            ConvergenceDecisionKind::Block
        }
    }
}
