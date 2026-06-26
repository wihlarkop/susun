//! Safe replacement convergence planning.

use indexmap::IndexSet;
use susun_engine::{ResourceName, ServiceInstanceId};
use susun_planner::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, CreateContainerAction, PlanAction,
    PlanActionNode, RecreateContainerAction, RemoveContainerAction, RenameContainerAction,
    StartContainerAction, StopContainerAction, VerifyReplacementAction,
};

use crate::ReplacementStrategy;

use super::ConvergencePlanFragment;

/// Inputs required to generate a replacement subgraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementInput {
    /// Service instance being replaced.
    pub instance: ServiceInstanceId,
    /// New container create action.
    pub create: CreateContainerAction,
    /// Existing runtime container name.
    pub existing_name: ResourceName,
    /// Name to use if the old container is renamed instead of removed first.
    pub renamed_existing_name: Option<ResourceName>,
    /// Replacement strategy.
    pub strategy: ReplacementStrategy,
}

/// Generates a conservative replacement fragment.
pub fn plan_replacement(input: ReplacementInput) -> ConvergencePlanFragment {
    let mut fragment = ConvergencePlanFragment::new();

    let marker = node(
        &input.instance,
        PlanAction::RecreateContainer(RecreateContainerAction {
            identity: input.instance.clone(),
            strategy: format!("{:?}", input.strategy),
        }),
        ActionSafety::Caution,
        "service instance requires replacement",
        IndexSet::new(),
    );
    let marker_id = marker.id.clone();
    fragment.insert(marker);

    let stop = node(
        &input.instance,
        PlanAction::StopContainer(StopContainerAction {
            identity: input.instance.clone(),
        }),
        ActionSafety::Caution,
        "old service instance must stop before replacement",
        deps([marker_id.clone()]),
    );
    let stop_id = stop.id.clone();
    fragment.insert(stop);

    let clear_name = match input.strategy {
        ReplacementStrategy::CreateThenSwitch => input.renamed_existing_name.map(|to| {
            node(
                &input.instance,
                PlanAction::RenameContainer(RenameContainerAction {
                    identity: input.instance.clone(),
                    from: input.existing_name.clone(),
                    to,
                }),
                ActionSafety::Caution,
                "old service instance is renamed to free the runtime name",
                deps([stop_id.clone()]),
            )
        }),
        ReplacementStrategy::StopRemoveCreateStart => Some(node(
            &input.instance,
            PlanAction::RemoveContainer(RemoveContainerAction {
                identity: input.instance.clone(),
            }),
            ActionSafety::Destructive,
            "old service instance is removed before replacement",
            deps([stop_id.clone()]),
        )),
        ReplacementStrategy::RestartOnly => None,
    };

    let create_deps = if let Some(clear_name) = clear_name {
        let id = clear_name.id.clone();
        fragment.insert(clear_name);
        deps([id])
    } else {
        deps([stop_id])
    };

    let create = node(
        &input.instance,
        PlanAction::CreateContainer(Box::new(input.create)),
        ActionSafety::Safe,
        "new service instance is created from desired configuration",
        create_deps,
    );
    let create_id = create.id.clone();
    fragment.insert(create);

    let start = node(
        &input.instance,
        PlanAction::StartContainer(StartContainerAction {
            identity: input.instance.clone(),
        }),
        ActionSafety::Safe,
        "new service instance is started",
        deps([create_id]),
    );
    let start_id = start.id.clone();
    fragment.insert(start);

    fragment.insert(node(
        &input.instance,
        PlanAction::VerifyReplacement(VerifyReplacementAction {
            identity: input.instance.clone(),
        }),
        ActionSafety::Safe,
        "replacement service instance is verified",
        deps([start_id]),
    ));

    fragment
}

fn node(
    instance: &ServiceInstanceId,
    action: PlanAction,
    safety: ActionSafety,
    message: &'static str,
    dependencies: IndexSet<ActionId>,
) -> PlanActionNode {
    PlanActionNode {
        id: action_id(instance, action.kind()),
        action,
        dependencies,
        reason: ActionExplanation::new(ActionReason::ServiceRequested, message),
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

fn deps<const N: usize>(ids: [ActionId; N]) -> IndexSet<ActionId> {
    ids.into_iter().collect()
}
