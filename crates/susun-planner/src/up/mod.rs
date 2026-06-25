//! `up` planner.

pub mod resources;
pub mod services;

use indexmap::IndexMap;
use susun_diagnostics::DiagnosticReport;

use crate::{
    ActionId, ExecutionPlan, NamingPolicy, PlanActionNode, PlanError, PlanOutcome,
    PlannedOperation, PlanningInput, UpPlanOptions, check_up_capabilities, validate_action_dag,
};

pub use resources::UpResourceActions;

/// Creates a deterministic dry-run `up` plan.
pub fn plan_up(
    input: &PlanningInput<'_>,
    options: UpPlanOptions,
    naming: &dyn NamingPolicy,
) -> Result<PlanOutcome, PlanError> {
    let diagnostics = check_up_capabilities(input, options);
    if diagnostics.has_errors() {
        return Ok(PlanOutcome::blocked(diagnostics));
    }

    let mut diagnostics = DiagnosticReport::new();
    let mut actions = IndexMap::new();
    let resources = resources::plan_prerequisite_resources(
        input,
        options,
        naming,
        &mut actions,
        &mut diagnostics,
    )?;
    if diagnostics.has_errors() {
        return Ok(PlanOutcome::blocked(diagnostics));
    }

    services::plan_services(input, naming, &resources, &mut actions, &mut diagnostics)?;

    if diagnostics.has_errors() {
        return Ok(PlanOutcome::blocked(diagnostics));
    }

    validate_action_dag(&actions)?;
    let plan = ExecutionPlan::new(
        input.identity.clone(),
        PlannedOperation::Up,
        actions,
        DiagnosticReport::new(),
    );

    Ok(PlanOutcome::planned(plan, diagnostics))
}

fn insert_action(
    actions: &mut IndexMap<ActionId, PlanActionNode>,
    node: PlanActionNode,
) -> Result<ActionId, PlanError> {
    let id = node.id.clone();
    if actions.insert(id.clone(), node).is_some() {
        return Err(PlanError::ActionIdCollision { id });
    }
    Ok(id)
}
