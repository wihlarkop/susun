//! `down` planner.

use indexmap::{IndexMap, IndexSet};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::SnapshotCompleteness;

use crate::ownership::service_identity_for;
use crate::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, DownPlanOptions, ExecutionPlan,
    NoOpAction, PlanAction, PlanActionNode, PlanError, PlanOutcome, PlannedOperation,
    PlanningInput, RemoveContainerAction, RemoveNetworkAction, RemoveVolumeAction,
    StopContainerAction, index_owned_resources, validate_action_dag,
};

const INCOMPLETE_SNAPSHOT: &str = "SUS-PLAN-020";

/// Creates a deterministic dry-run `down` plan.
pub fn plan_down(
    input: &PlanningInput<'_>,
    options: DownPlanOptions,
) -> Result<PlanOutcome, PlanError> {
    let mut diagnostics = DiagnosticReport::new();
    let owned = index_owned_resources(input, &mut diagnostics);
    if diagnostics.has_errors() {
        return Ok(PlanOutcome::blocked(diagnostics));
    }

    let mut actions = IndexMap::new();
    let mut remove_container_ids = IndexMap::new();

    for service_name in input.graph.order.iter().rev() {
        if !input.selection.active_services.contains(service_name) {
            continue;
        }
        let Some(container_ids) = owned.containers_by_service.get(service_name) else {
            continue;
        };
        if container_ids.is_empty() {
            continue;
        }

        let identity = service_identity_for(&input.identity.working_set, service_name);
        if container_snapshot_incomplete(input, container_ids) {
            diagnostics.push(Diagnostic::new(
                INCOMPLETE_SNAPSHOT,
                Severity::Error,
                format!(
                    "cannot safely remove service '{}' from incomplete snapshot",
                    service_name.as_str()
                ),
            ));
            continue;
        }

        let stop_action = PlanAction::StopContainer(StopContainerAction {
            identity: identity.clone(),
        });
        let stop_id = make_down_action_id(input, &stop_action, "0");
        let stop_node = node(
            stop_id,
            stop_action,
            ActionReason::TeardownRequested,
            "selected service should be stopped",
            ActionSafety::Destructive,
        );
        let stop_id = insert_action(&mut actions, stop_node)?;

        let remove_action = PlanAction::RemoveContainer(RemoveContainerAction { identity });
        let remove_id = make_down_action_id(input, &remove_action, "0");
        let mut remove_node = node(
            remove_id,
            remove_action,
            ActionReason::TeardownRequested,
            "selected service container should be removed",
            ActionSafety::Destructive,
        );
        remove_node.dependencies.insert(stop_id);
        let remove_id = insert_action(&mut actions, remove_node)?;
        remove_container_ids.insert(service_name.clone(), remove_id);
    }

    if diagnostics.has_errors() {
        return Ok(PlanOutcome::blocked(diagnostics));
    }

    plan_network_removal(input, &owned.networks, &remove_container_ids, &mut actions)?;
    if options.remove_volumes {
        plan_volume_removal(input, &owned.volumes, &remove_container_ids, &mut actions)?;
    }

    if actions.is_empty() {
        let action = PlanAction::NoOp(NoOpAction {
            resource: "project".to_owned(),
            description: "no owned resources require teardown".to_owned(),
        });
        let id = make_down_action_id(input, &action, "0");
        let no_op = node(
            id,
            action,
            ActionReason::ExistingResourceAccepted,
            "project is already down",
            ActionSafety::Safe,
        );
        insert_action(&mut actions, no_op)?;
    }

    validate_action_dag(&actions)?;
    let plan = ExecutionPlan::new(
        input.identity.clone(),
        PlannedOperation::Down,
        actions,
        DiagnosticReport::new(),
    );

    Ok(PlanOutcome::planned(plan, diagnostics))
}

fn plan_network_removal(
    input: &PlanningInput<'_>,
    network_ids: &IndexSet<susun_engine::NetworkId>,
    remove_container_ids: &IndexMap<susun_model::ServiceName, ActionId>,
    actions: &mut IndexMap<ActionId, PlanActionNode>,
) -> Result<(), PlanError> {
    for network_id in network_ids {
        let Some(network) = input.snapshot.networks.get(network_id) else {
            continue;
        };
        let Some(identity) = network.network_identity.clone() else {
            continue;
        };
        let action = PlanAction::RemoveNetwork(RemoveNetworkAction { identity });
        let id = make_down_action_id(input, &action, network_id.as_str());
        let mut action_node = node(
            id,
            action,
            ActionReason::TeardownRequested,
            "owned project network should be removed",
            ActionSafety::Destructive,
        );
        action_node
            .dependencies
            .extend(remove_container_ids.values().cloned());
        insert_action(actions, action_node)?;
    }

    Ok(())
}

fn plan_volume_removal(
    input: &PlanningInput<'_>,
    volume_ids: &IndexSet<susun_engine::VolumeId>,
    remove_container_ids: &IndexMap<susun_model::ServiceName, ActionId>,
    actions: &mut IndexMap<ActionId, PlanActionNode>,
) -> Result<(), PlanError> {
    for volume_id in volume_ids {
        let Some(volume) = input.snapshot.volumes.get(volume_id) else {
            continue;
        };
        let Some(identity) = volume.volume_identity.clone() else {
            continue;
        };
        let action = PlanAction::RemoveVolume(RemoveVolumeAction { identity });
        let id = make_down_action_id(input, &action, volume_id.as_str());
        let mut action_node = node(
            id,
            action,
            ActionReason::TeardownRequested,
            "owned project volume should be removed",
            ActionSafety::Destructive,
        );
        action_node
            .dependencies
            .extend(remove_container_ids.values().cloned());
        insert_action(actions, action_node)?;
    }

    Ok(())
}

fn container_snapshot_incomplete(
    input: &PlanningInput<'_>,
    container_ids: &[susun_engine::ContainerId],
) -> bool {
    container_ids.iter().any(|id| {
        input
            .snapshot
            .containers
            .get(id)
            .is_some_and(|container| container.completeness != SnapshotCompleteness::Complete)
    })
}

fn make_down_action_id(
    input: &PlanningInput<'_>,
    action: &PlanAction,
    discriminator: &str,
) -> ActionId {
    ActionId::from_parts(&[
        "1",
        input.identity.working_set.as_str(),
        PlannedOperation::Down.as_str(),
        &action.resource_key(),
        action.kind(),
        discriminator,
    ])
}

fn node(
    id: ActionId,
    action: PlanAction,
    reason: ActionReason,
    message: &str,
    safety: ActionSafety,
) -> PlanActionNode {
    PlanActionNode {
        id,
        action,
        dependencies: IndexSet::new(),
        reason: ActionExplanation::new(reason, message),
        safety,
    }
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
