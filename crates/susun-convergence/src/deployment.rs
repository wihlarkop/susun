//! Desired and observed deployment models.

use indexmap::{IndexMap, IndexSet};
use susun_engine::{
    EngineSnapshot, NetworkIdentity, ObservedContainer, ObservedNetwork, ObservedVolume,
    ProjectIdentity, ResourceName, ServiceInstanceId, VolumeIdentity,
};
use susun_graph::DependencyGraph;
use susun_model::{Project, ServiceName};
use susun_normalize::selection::ProjectSelection;

/// Desired replica count for one service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct DesiredReplicaCount(u32);

impl DesiredReplicaCount {
    /// One desired replica.
    pub const ONE: Self = Self(1);

    /// Creates a desired replica count.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the replica count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Default for DesiredReplicaCount {
    fn default() -> Self {
        Self::ONE
    }
}

/// Desired deployment state used by convergence.
#[derive(Debug, Clone)]
pub struct DesiredDeployment {
    /// Canonical project.
    pub project: Project,
    /// Active project selection.
    pub selection: ProjectSelection,
    /// Dependency graph.
    pub graph: DependencyGraph,
    /// Project identity.
    pub identity: ProjectIdentity,
    /// Desired replicas by service.
    pub replicas: IndexMap<ServiceName, DesiredReplicaCount>,
}

impl DesiredDeployment {
    /// Creates a desired deployment, filling omitted active services with one replica.
    pub fn new(
        project: Project,
        selection: ProjectSelection,
        graph: DependencyGraph,
        identity: ProjectIdentity,
        mut replicas: IndexMap<ServiceName, DesiredReplicaCount>,
    ) -> Self {
        for service in &selection.active_services {
            replicas
                .entry(service.clone())
                .or_insert(DesiredReplicaCount::ONE);
        }
        Self {
            project,
            selection,
            graph,
            identity,
            replicas,
        }
    }
}

/// Observed deployment state used by convergence.
#[derive(Debug, Clone)]
pub struct ObservedDeployment {
    /// Neutral engine snapshot.
    pub snapshot: EngineSnapshot,
    /// Ownership index derived from the snapshot.
    pub ownership: OwnershipIndex,
}

impl ObservedDeployment {
    /// Creates an observed deployment by indexing a snapshot.
    pub fn new(snapshot: EngineSnapshot, desired_services: &IndexSet<ServiceName>) -> Self {
        let ownership = OwnershipIndex::from_snapshot(&snapshot, desired_services);
        Self {
            snapshot,
            ownership,
        }
    }
}

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
        let mut index = Self::default();
        let mut duplicates: IndexMap<ServiceInstanceId, Vec<ObservedContainer>> = IndexMap::new();

        for container in snapshot.containers.values() {
            let Some(instance) = &container.service_identity else {
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
