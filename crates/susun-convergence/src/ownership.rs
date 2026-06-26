//! Observed resource ownership indexing.

use indexmap::{IndexMap, IndexSet};
use susun_engine::{
    EngineSnapshot, NetworkIdentity, ObservedContainer, ObservedNetwork, ObservedVolume,
    ResourceName, ServiceInstanceId, VolumeIdentity,
};
use susun_model::ServiceName;

/// Ownership conflict found while indexing observed resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipConflict {
    /// Claimed service instance.
    pub instance: ServiceInstanceId,
    /// Conflicting containers.
    pub containers: Vec<ObservedContainer>,
}

/// Index of observed resources owned by one project.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OwnershipIndex {
    /// Containers keyed by claimed service instance.
    pub containers: IndexMap<ServiceInstanceId, ObservedContainer>,
    /// Duplicate service-instance claims.
    pub duplicate_claims: Vec<OwnershipConflict>,
    /// Owned containers not represented by desired services.
    pub orphan_containers: Vec<ObservedContainer>,
    /// Networks keyed by declared identity.
    pub project_networks: IndexMap<NetworkIdentity, ObservedNetwork>,
    /// Volumes keyed by declared identity.
    pub project_volumes: IndexMap<VolumeIdentity, ObservedVolume>,
    /// Observed resources whose runtime name collides with desired runtime names but are not owned.
    pub foreign_name_conflicts: Vec<ResourceName>,
}

impl OwnershipIndex {
    /// Builds an ownership index from a neutral snapshot.
    pub fn from_snapshot(
        snapshot: &EngineSnapshot,
        desired_services: &IndexSet<ServiceName>,
    ) -> Self {
        Self::from_snapshot_with_names(snapshot, desired_services, &IndexSet::new())
    }

    /// Builds an ownership index and records foreign runtime-name collisions.
    pub fn from_snapshot_with_names(
        snapshot: &EngineSnapshot,
        desired_services: &IndexSet<ServiceName>,
        desired_container_names: &IndexSet<ResourceName>,
    ) -> Self {
        let mut index = Self::default();
        let mut duplicates: IndexMap<ServiceInstanceId, Vec<ObservedContainer>> = IndexMap::new();

        for container in snapshot.containers.values() {
            let Some(instance) = &container.service_identity else {
                if desired_container_names.contains(&container.name) {
                    index.foreign_name_conflicts.push(container.name.clone());
                }
                continue;
            };

            if desired_services.contains(&instance.service) {
                if let Some(existing) = index.containers.get(instance) {
                    duplicates
                        .entry(instance.clone())
                        .or_default()
                        .extend([existing.clone(), container.clone()]);
                } else {
                    index.containers.insert(instance.clone(), container.clone());
                }
            } else {
                index.orphan_containers.push(container.clone());
            }
        }

        for (instance, containers) in duplicates {
            index.containers.shift_remove(&instance);
            index.duplicate_claims.push(OwnershipConflict {
                instance,
                containers,
            });
        }

        for network in snapshot.networks.values() {
            if let Some(identity) = &network.network_identity {
                index
                    .project_networks
                    .insert(identity.clone(), network.clone());
            }
        }

        for volume in snapshot.volumes.values() {
            if let Some(identity) = &volume.volume_identity {
                index
                    .project_volumes
                    .insert(identity.clone(), volume.clone());
            }
        }

        index
    }
}
