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

#[cfg(test)]
mod tests {
    use std::{error::Error, time::SystemTime};

    use indexmap::{IndexMap, IndexSet};
    use susun_engine::{
        ContainerId, ContainerState, ImageId, ObservedContainer, ObservedImageRef,
        ProjectInstanceId, ReplicaIndex, ResourceName, ServiceInstanceId, SnapshotCompleteness,
    };

    use super::*;

    #[test]
    fn duplicate_owned_container_claims_are_not_indexed_as_safe_owners()
    -> Result<(), Box<dyn Error>> {
        let service = ServiceName::new("web");
        let project = ProjectInstanceId::new("project-a")?;
        let instance =
            ServiceInstanceId::new(project.clone(), service.clone(), ReplicaIndex::FIRST);
        let snapshot = EngineSnapshot {
            observed_at: SystemTime::UNIX_EPOCH,
            containers: IndexMap::from_iter([
                (
                    ContainerId::new("one")?,
                    observed_container("one", "web-1", Some(instance.clone()))?,
                ),
                (
                    ContainerId::new("two")?,
                    observed_container("two", "web-2", Some(instance.clone()))?,
                ),
            ]),
            networks: IndexMap::new(),
            volumes: IndexMap::new(),
            images: IndexMap::new(),
        };

        let index = OwnershipIndex::from_snapshot(&snapshot, &IndexSet::from_iter([service]));

        assert!(!index.containers.contains_key(&instance));
        assert_eq!(index.duplicate_claims.len(), 1);
        assert_eq!(index.duplicate_claims[0].containers.len(), 2);
        Ok(())
    }

    #[test]
    fn foreign_name_conflicts_are_tracked_separately_from_owned_resources()
    -> Result<(), Box<dyn Error>> {
        let desired_name = ResourceName::new("web-1")?;
        let snapshot = EngineSnapshot {
            observed_at: SystemTime::UNIX_EPOCH,
            containers: IndexMap::from_iter([(
                ContainerId::new("foreign")?,
                observed_container("foreign", desired_name.as_str(), None)?,
            )]),
            networks: IndexMap::new(),
            volumes: IndexMap::new(),
            images: IndexMap::new(),
        };

        let index = OwnershipIndex::from_snapshot_with_names(
            &snapshot,
            &IndexSet::new(),
            &IndexSet::from_iter([desired_name.clone()]),
        );

        assert!(index.containers.is_empty());
        assert_eq!(index.foreign_name_conflicts, vec![desired_name]);
        Ok(())
    }

    fn observed_container(
        id: &str,
        name: &str,
        service_identity: Option<ServiceInstanceId>,
    ) -> Result<ObservedContainer, Box<dyn Error>> {
        Ok(ObservedContainer {
            id: ContainerId::new(id)?,
            name: ResourceName::new(name)?,
            state: ContainerState::Running,
            health: None,
            image: ObservedImageRef::Id(ImageId::new("image")?),
            labels: IndexMap::new(),
            project_identity: service_identity
                .as_ref()
                .map(|identity| identity.project.clone()),
            service_identity,
            configuration_fingerprint: None,
            completeness: SnapshotCompleteness::Complete,
        })
    }
}
