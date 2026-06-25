//! Neutral engine snapshot model.

use std::time::SystemTime;

use indexmap::IndexMap;
use susun_model::ImageRef;

use crate::{
    ConfigurationFingerprint, ContainerId, ImageId, LabelKey, LabelValue, NetworkId,
    NetworkIdentity, ProjectInstanceId, ResourceName, ServiceInstanceId, VolumeId, VolumeIdentity,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Snapshot of observed engine state.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineSnapshot {
    /// Time at which the snapshot was observed.
    pub observed_at: SystemTime,
    /// Observed containers keyed by engine ID.
    pub containers: IndexMap<ContainerId, ObservedContainer>,
    /// Observed networks keyed by engine ID.
    pub networks: IndexMap<NetworkId, ObservedNetwork>,
    /// Observed volumes keyed by engine ID.
    pub volumes: IndexMap<VolumeId, ObservedVolume>,
    /// Observed images keyed by engine ID.
    pub images: IndexMap<ImageId, ObservedImage>,
}

impl EngineSnapshot {
    /// Creates an empty snapshot.
    pub fn empty(observed_at: SystemTime) -> Self {
        Self {
            observed_at,
            containers: IndexMap::new(),
            networks: IndexMap::new(),
            volumes: IndexMap::new(),
            images: IndexMap::new(),
        }
    }

    /// Returns the deterministic portion of this snapshot.
    pub fn stable_projection(&self) -> StableEngineSnapshot {
        StableEngineSnapshot {
            containers: self.containers.clone(),
            networks: self.networks.clone(),
            volumes: self.volumes.clone(),
            images: self.images.clone(),
        }
    }
}

/// Deterministic snapshot projection that excludes observational metadata.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StableEngineSnapshot {
    /// Observed containers keyed by engine ID.
    pub containers: IndexMap<ContainerId, ObservedContainer>,
    /// Observed networks keyed by engine ID.
    pub networks: IndexMap<NetworkId, ObservedNetwork>,
    /// Observed volumes keyed by engine ID.
    pub volumes: IndexMap<VolumeId, ObservedVolume>,
    /// Observed images keyed by engine ID.
    pub images: IndexMap<ImageId, ObservedImage>,
}

/// Observed container state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObservedContainer {
    /// Engine container ID.
    pub id: ContainerId,
    /// Runtime container name.
    pub name: ResourceName,
    /// Runtime state.
    pub state: ContainerState,
    /// Optional health state.
    pub health: Option<HealthState>,
    /// Image used by the container.
    pub image: ObservedImageRef,
    /// Runtime labels.
    pub labels: IndexMap<LabelKey, LabelValue>,
    /// Project ownership identity, if known.
    pub project_identity: Option<ProjectInstanceId>,
    /// Service ownership identity, if known.
    pub service_identity: Option<ServiceInstanceId>,
    /// Optional normalized configuration fingerprint.
    pub configuration_fingerprint: Option<ConfigurationFingerprint>,
    /// Completeness of this observed record.
    pub completeness: SnapshotCompleteness,
}

/// Observed network state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObservedNetwork {
    /// Engine network ID.
    pub id: NetworkId,
    /// Runtime network name.
    pub name: ResourceName,
    /// Runtime labels.
    pub labels: IndexMap<LabelKey, LabelValue>,
    /// Project ownership identity, if known.
    pub project_identity: Option<ProjectInstanceId>,
    /// Network ownership identity, if known.
    pub network_identity: Option<NetworkIdentity>,
    /// Completeness of this observed record.
    pub completeness: SnapshotCompleteness,
}

/// Observed volume state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObservedVolume {
    /// Engine volume ID.
    pub id: VolumeId,
    /// Runtime volume name.
    pub name: ResourceName,
    /// Runtime labels.
    pub labels: IndexMap<LabelKey, LabelValue>,
    /// Project ownership identity, if known.
    pub project_identity: Option<ProjectInstanceId>,
    /// Volume ownership identity, if known.
    pub volume_identity: Option<VolumeIdentity>,
    /// Completeness of this observed record.
    pub completeness: SnapshotCompleteness,
}

/// Observed image state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObservedImage {
    /// Engine image ID.
    pub id: ImageId,
    /// Image references known for this image.
    pub references: Vec<ImageRef>,
    /// Runtime labels.
    pub labels: IndexMap<LabelKey, LabelValue>,
    /// Completeness of this observed record.
    pub completeness: SnapshotCompleteness,
}

/// Container image reference as observed from the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ObservedImageRef {
    /// Image resolved to an engine ID.
    Id(ImageId),
    /// Image known only by reference string.
    Reference(ImageRef),
    /// Image could not be observed.
    Unknown,
}

/// Container runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ContainerState {
    /// Container exists but is not running.
    Created,
    /// Container is running.
    Running,
    /// Container exited.
    Exited,
    /// Container is paused.
    Paused,
    /// Container is restarting.
    Restarting,
    /// Container state is unknown.
    Unknown,
}

/// Container health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum HealthState {
    /// Healthcheck is starting.
    Starting,
    /// Container is healthy.
    Healthy,
    /// Container is unhealthy.
    Unhealthy,
    /// Health state is unknown.
    Unknown,
}

/// Completeness of observed snapshot data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SnapshotCompleteness {
    /// All required fields were observed.
    Complete,
    /// Some fields could not be observed.
    Partial {
        /// Unavailable fields.
        unavailable_fields: Vec<SnapshotField>,
    },
    /// Completeness could not be determined.
    #[default]
    Unknown,
}

/// Snapshot field names used in partial-completeness records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SnapshotField {
    /// Runtime name.
    Name,
    /// Runtime labels.
    Labels,
    /// Resource state.
    State,
    /// Resource health.
    Health,
    /// Image information.
    Image,
    /// Ownership identity.
    Ownership,
    /// Configuration fingerprint.
    ConfigurationFingerprint,
}
