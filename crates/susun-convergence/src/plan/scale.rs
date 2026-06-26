//! Scale convergence planning.

use indexmap::IndexSet;
use susun_engine::ServiceInstanceId;
use susun_planner::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, PlanAction, PlanActionNode,
    ScaleDownReplicaAction, ScaleUpReplicaAction,
};

use crate::{ConvergencePlanFragment, ScaleDelta};

/// Builds scale-up and scale-down marker actions.
pub fn plan_scale(delta: &ScaleDelta) -> ConvergencePlanFragment {
    let mut fragment = ConvergencePlanFragment::new();
    for instance in &delta.scale_up {
        fragment.insert(node(
            instance,
            PlanAction::ScaleUpReplica(ScaleUpReplicaAction {
                identity: instance.clone(),
            }),
            ActionSafety::Safe,
            "missing replica should be created",
        ));
    }
    for instance in &delta.scale_down {
        fragment.insert(node(
            instance,
            PlanAction::ScaleDownReplica(ScaleDownReplicaAction {
                identity: instance.clone(),
            }),
            ActionSafety::Destructive,
            "extra replica should be removed",
        ));
    }
    fragment
}

fn node(
    instance: &ServiceInstanceId,
    action: PlanAction,
    safety: ActionSafety,
    message: &'static str,
) -> PlanActionNode {
    PlanActionNode {
        id: ActionId::from_parts(&[
            "converge",
            action.kind(),
            instance.project.as_str(),
            instance.service.as_str(),
            &instance.replica.as_u32().to_string(),
        ]),
        action,
        dependencies: IndexSet::new(),
        reason: ActionExplanation::new(ActionReason::ServiceRequested, message),
        safety,
    }
}
