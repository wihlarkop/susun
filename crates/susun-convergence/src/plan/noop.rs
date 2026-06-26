//! No-op and start-only convergence planning.

use indexmap::IndexSet;
use susun_engine::ServiceInstanceId;
use susun_planner::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, NoOpAction, PlanAction,
    PlanActionNode, StartContainerAction,
};

use crate::InstanceDifference;

/// Plans an action for unchanged or stopped-compatible instances.
pub fn plan_noop_or_start(
    instance: &ServiceInstanceId,
    difference: &InstanceDifference,
) -> Option<PlanActionNode> {
    match difference {
        InstanceDifference::Unchanged => Some(node(
            instance,
            PlanAction::NoOp(NoOpAction {
                resource: format!("service:{}", instance.service.as_str()),
                description: "service instance already converged".to_string(),
            }),
            ActionSafety::Safe,
            "no mutation is required",
        )),
        InstanceDifference::StoppedButCompatible => Some(node(
            instance,
            PlanAction::StartContainer(StartContainerAction {
                identity: instance.clone(),
            }),
            ActionSafety::Safe,
            "compatible service instance is stopped",
        )),
        _ => None,
    }
}

fn node(
    instance: &ServiceInstanceId,
    action: PlanAction,
    safety: ActionSafety,
    message: &'static str,
) -> PlanActionNode {
    PlanActionNode {
        id: action_id(instance, action.kind()),
        action,
        dependencies: IndexSet::new(),
        reason: ActionExplanation::new(ActionReason::ExistingResourceAccepted, message),
        safety,
    }
}

fn action_id(instance: &ServiceInstanceId, kind: &str) -> ActionId {
    ActionId::from_parts(&[
        "converge",
        kind,
        instance.project.as_str(),
        instance.service.as_str(),
        &instance.replica.as_u32().to_string(),
    ])
}
