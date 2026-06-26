//! Execution plan validation.

use susun_planner::{ExecutionPlan, PlanAction, PlanSchemaVersion, validate_action_dag};

use crate::PlanValidationError;

/// Validates an immutable plan before execution.
pub fn validate_plan_for_execution(plan: &ExecutionPlan) -> Result<(), PlanValidationError> {
    if plan.schema_version.major != PlanSchemaVersion::CURRENT.major {
        return Err(PlanValidationError::UnsupportedSchema {
            major: plan.schema_version.major,
            minor: plan.schema_version.minor,
        });
    }
    if plan.actions.is_empty() {
        return Err(PlanValidationError::EmptyPlan {
            plan_id: plan.plan_id.clone(),
        });
    }
    validate_action_dag(&plan.actions)?;
    for (id, node) in &plan.actions {
        if matches!(node.action, PlanAction::NoOp(_)) {
            continue;
        }
        if node.action.kind().is_empty() {
            return Err(PlanValidationError::UnsupportedAction {
                action_id: id.clone(),
                reason: "empty action kind".to_owned(),
            });
        }
    }
    Ok(())
}
