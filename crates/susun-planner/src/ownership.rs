//! Ownership indexing for observed resources.

use indexmap::{IndexMap, IndexSet};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::{ContainerId, NetworkId, ProjectInstanceId, ServiceInstanceId, VolumeId};
use susun_model::ServiceName;

use crate::PlanningInput;

const DUPLICATE_OWNERSHIP: &str = "SUS-PLAN-010";

/// Observed resources owned by the requested project instance.
#[derive(Debug, Clone, Default)]
pub struct OwnedResourceIndex {
    /// Owned containers by service name.
    pub containers_by_service: IndexMap<ServiceName, Vec<ContainerId>>,
    /// Owned network IDs.
    pub networks: IndexSet<NetworkId>,
    /// Owned volume IDs.
    pub volumes: IndexSet<VolumeId>,
}

/// Indexes observed resources with matching Susun ownership labels.
pub fn index_owned_resources(
    input: &PlanningInput<'_>,
    diagnostics: &mut DiagnosticReport,
) -> OwnedResourceIndex {
    let mut index = OwnedResourceIndex::default();
    let project = &input.identity.working_set;

    for container in input.snapshot.containers.values() {
        if container.project_identity.as_ref() != Some(project) {
            continue;
        }
        let Some(service_identity) = &container.service_identity else {
            continue;
        };
        if &service_identity.project != project {
            continue;
        }
        index
            .containers_by_service
            .entry(service_identity.service.clone())
            .or_default()
            .push(container.id.clone());
    }

    for (service, containers) in &index.containers_by_service {
        if containers.len() > 1 {
            diagnostics.push(Diagnostic::new(
                DUPLICATE_OWNERSHIP,
                Severity::Error,
                format!(
                    "multiple observed containers claim service '{}'",
                    service.as_str()
                ),
            ));
        }
    }

    collect_project_resources(project, input, &mut index);
    index
}

fn collect_project_resources(
    project: &ProjectInstanceId,
    input: &PlanningInput<'_>,
    index: &mut OwnedResourceIndex,
) {
    for network in input.snapshot.networks.values() {
        if network.project_identity.as_ref() == Some(project) {
            index.networks.insert(network.id.clone());
        }
    }

    for volume in input.snapshot.volumes.values() {
        if volume.project_identity.as_ref() == Some(project) {
            index.volumes.insert(volume.id.clone());
        }
    }
}

/// Returns a first-replica service identity.
pub(crate) fn service_identity_for(
    project: &ProjectInstanceId,
    service: &ServiceName,
) -> ServiceInstanceId {
    ServiceInstanceId::new(
        project.clone(),
        service.clone(),
        susun_engine::ReplicaIndex::FIRST,
    )
}
