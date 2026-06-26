//! Service action planning for `up`.

use indexmap::{IndexMap, IndexSet};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::{ReplicaIndex, ResourceName, ServiceInstanceId};
use susun_model::{DependencyCondition, NetworkAttachment, VolumeKind, volume::CanonicalVolume};

use crate::{
    ActionId, ActionReason, ActionSafety, CreateContainerAction, NamingPolicy, PlanAction,
    PlanActionNode, PlanError, PlanningInput, StartContainerAction, WaitForDependencyAction,
};

use super::{
    UpResourceActions, insert_action,
    resources::{make_action_id, node},
};

const DUPLICATE_CONTAINER: &str = "SUS-PLAN-002";

pub(crate) fn plan_services(
    input: &PlanningInput<'_>,
    naming: &dyn NamingPolicy,
    resources: &UpResourceActions,
    actions: &mut IndexMap<ActionId, PlanActionNode>,
    diagnostics: &mut DiagnosticReport,
) -> Result<(), PlanError> {
    let mut start_ids: IndexMap<susun_model::ServiceName, ActionId> = IndexMap::new();

    for service_name in &input.graph.order {
        if !input.selection.active_services.contains(service_name) {
            continue;
        }

        let Some(service) = input.project.services.get(service_name) else {
            continue;
        };
        let identity = ServiceInstanceId::new(
            input.identity.working_set.clone(),
            service_name.clone(),
            ReplicaIndex::FIRST,
        );
        let runtime_name =
            naming
                .container_name(&identity)
                .map_err(|err| PlanError::InvariantViolation {
                    detail: err.to_string(),
                })?;

        if duplicate_container_claim(input, &identity) {
            diagnostics.push(Diagnostic::new(
                DUPLICATE_CONTAINER,
                Severity::Error,
                format!(
                    "multiple observed containers claim service '{}'",
                    service_name.as_str()
                ),
            ));
            continue;
        }

        let create_action = PlanAction::CreateContainer(Box::new(CreateContainerAction {
            identity: identity.clone(),
            name: runtime_name,
            image: service.image.clone(),
            command: service.command.clone(),
            entrypoint: service.entrypoint.clone(),
            environment: service.environment.clone(),
            labels: service.labels.clone(),
            ports: service.ports.clone(),
            volumes: runtime_volumes(service, resources),
            networks: runtime_networks(service, resources),
            healthcheck: service.healthcheck.clone(),
            restart: service.restart.clone(),
        }));
        let create_id = make_action_id(input, &create_action, "0");
        let mut create_node = node(
            create_id,
            create_action,
            ActionReason::ServiceRequested,
            "selected service requires a container",
            ActionSafety::Safe,
        );

        add_service_resource_dependencies(
            service_name,
            service,
            resources,
            &mut create_node.dependencies,
        );

        let create_id = insert_action(actions, create_node)?;
        let start_action = PlanAction::StartContainer(StartContainerAction {
            identity: identity.clone(),
        });
        let start_id = make_action_id(input, &start_action, "0");
        let mut start_node = node(
            start_id,
            start_action,
            ActionReason::ServiceRequested,
            "selected service should be started",
            ActionSafety::Safe,
        );
        start_node.dependencies.insert(create_id);

        for dependency_name in service.depends_on.keys() {
            if let Some(dependency_start) = start_ids.get(dependency_name) {
                start_node.dependencies.insert(dependency_start.clone());
            }
        }

        let start_id = insert_action(actions, start_node)?;
        start_ids.insert(service_name.clone(), start_id);

        add_wait_actions(input, service_name, &identity, actions, &mut start_ids)?;
    }

    Ok(())
}

fn runtime_networks(
    service: &susun_model::Service,
    resources: &UpResourceActions,
) -> IndexMap<ResourceName, NetworkAttachment> {
    service
        .networks
        .iter()
        .filter_map(|(name, attachment)| {
            resources
                .network_names
                .get(name.as_str())
                .cloned()
                .map(|runtime_name| (runtime_name, attachment.clone()))
        })
        .collect()
}

fn runtime_volumes(
    service: &susun_model::Service,
    resources: &UpResourceActions,
) -> Vec<CanonicalVolume> {
    service
        .volumes
        .iter()
        .cloned()
        .map(|mut volume| {
            if volume.kind == VolumeKind::Volume
                && let Some(source) = &volume.source
                && let Some(runtime_name) = resources.volume_names.get(source)
            {
                volume.source = Some(runtime_name.as_str().to_owned());
            }
            volume
        })
        .collect()
}

fn add_service_resource_dependencies(
    service_name: &susun_model::ServiceName,
    service: &susun_model::Service,
    resources: &UpResourceActions,
    dependencies: &mut IndexSet<ActionId>,
) {
    if let Some(action_id) = resources.builds.get(service_name.as_str()) {
        dependencies.insert(action_id.clone());
    }

    if let Some(image) = &service.image
        && let Some(action_id) = resources.images.get(image.as_str())
    {
        dependencies.insert(action_id.clone());
    }

    for network_name in service.networks.keys() {
        if let Some(action_id) = resources.networks.get(network_name.as_str()) {
            dependencies.insert(action_id.clone());
        }
    }

    for volume in &service.volumes {
        if volume.kind != VolumeKind::Volume {
            continue;
        }
        let Some(source) = &volume.source else {
            continue;
        };
        if let Some(action_id) = resources.volumes.get(source.as_str()) {
            dependencies.insert(action_id.clone());
        }
    }
}

fn add_wait_actions(
    input: &PlanningInput<'_>,
    service_name: &susun_model::ServiceName,
    identity: &ServiceInstanceId,
    actions: &mut IndexMap<ActionId, PlanActionNode>,
    start_ids: &mut IndexMap<susun_model::ServiceName, ActionId>,
) -> Result<(), PlanError> {
    let Some(service) = input.project.services.get(service_name) else {
        return Ok(());
    };

    for (dependency_name, dependency) in &service.depends_on {
        if dependency.condition == DependencyCondition::ServiceStarted {
            continue;
        }
        let Some(dependency_start) = start_ids.get(dependency_name) else {
            continue;
        };

        let dependency_identity = ServiceInstanceId::new(
            input.identity.working_set.clone(),
            dependency_name.clone(),
            ReplicaIndex::FIRST,
        );
        let wait_action = PlanAction::WaitForDependency(WaitForDependencyAction {
            dependent: identity.clone(),
            dependency: dependency_identity,
            condition: format!("{:?}", dependency.condition),
        });
        let wait_id = make_action_id(input, &wait_action, dependency_name.as_str());
        let mut wait_node = node(
            wait_id,
            wait_action,
            ActionReason::DependencyRequired,
            "service dependency condition must be satisfied",
            ActionSafety::Safe,
        );
        wait_node.dependencies.insert(dependency_start.clone());
        let wait_id = insert_action(actions, wait_node)?;

        if let Some(start_id) = start_ids.get(service_name)
            && let Some(start_node) = actions.get_mut(start_id)
        {
            start_node.dependencies.insert(wait_id);
        }
    }

    Ok(())
}

fn duplicate_container_claim(input: &PlanningInput<'_>, identity: &ServiceInstanceId) -> bool {
    input
        .snapshot
        .containers
        .values()
        .filter(|container| container.service_identity.as_ref() == Some(identity))
        .take(2)
        .count()
        > 1
}
