//! Display-safe engine-wide inventory contracts.

use crate::{
    ContainerId, ContainerState, EngineArchitecture, EngineOperatingSystem, EngineVersion,
    HealthState, ImageId, LabelKey, ObservedImageRef, ProjectInstanceId, ResourceName,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Schema version shared by engine-wide inventory responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineInventorySchemaVersion {
    /// Breaking schema generation.
    pub major: u16,
    /// Additive schema generation.
    pub minor: u16,
}

impl EngineInventorySchemaVersion {
    /// Current engine inventory schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Engine-wide container inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineContainerInventory {
    /// Serialized contract version.
    pub schema_version: EngineInventorySchemaVersion,
    /// Observation time as seconds since the Unix epoch.
    pub observed_at_epoch_seconds: u64,
    /// Containers ordered by opaque engine ID.
    pub containers: Vec<EngineContainerSummary>,
}

/// Display-safe engine-wide container summary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineContainerSummary {
    /// Opaque engine container ID.
    pub id: ContainerId,
    /// Runtime-visible container name.
    pub name: ResourceName,
    /// Runtime state.
    pub state: ContainerState,
    /// Health state when reported.
    pub health: Option<HealthState>,
    /// Image reference or ID reported by the engine.
    pub image: ObservedImageRef,
    /// Sorted runtime label keys. Values are intentionally excluded.
    pub label_keys: Vec<LabelKey>,
    /// Susun project identity inferred from labels, when present.
    pub project_identity: Option<ProjectInstanceId>,
    /// Creation time as seconds since the Unix epoch.
    pub created_at_epoch_seconds: Option<u64>,
    /// Writable-layer bytes when the engine calculated them.
    pub writable_size_bytes: Option<u64>,
    /// Root-filesystem bytes when the engine calculated them.
    pub root_filesystem_size_bytes: Option<u64>,
}

/// Engine-wide image inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineImageInventory {
    /// Serialized contract version.
    pub schema_version: EngineInventorySchemaVersion,
    /// Observation time as seconds since the Unix epoch.
    pub observed_at_epoch_seconds: u64,
    /// Images ordered by opaque engine ID.
    pub images: Vec<EngineImageSummary>,
}

/// Display-safe engine-wide image summary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineImageSummary {
    /// Opaque engine image ID.
    pub id: ImageId,
    /// Tag references reported by the engine.
    pub references: Vec<susun_model::ImageRef>,
    /// Content digests reported by the engine.
    pub digests: Vec<String>,
    /// Sorted runtime label keys. Values are intentionally excluded.
    pub label_keys: Vec<LabelKey>,
    /// Creation time as seconds since the Unix epoch.
    pub created_at_epoch_seconds: Option<u64>,
    /// Total image bytes.
    pub size_bytes: Option<u64>,
    /// Shared-layer bytes when calculated by the engine.
    pub shared_size_bytes: Option<u64>,
    /// Number of containers using the image when calculated by the engine.
    pub container_count: Option<u64>,
}

/// Display-safe engine and host information for diagnostics and capacity views.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineInformation {
    /// Serialized contract version.
    pub schema_version: EngineInventorySchemaVersion,
    /// Engine version when reported.
    pub engine_version: Option<EngineVersion>,
    /// Engine operating system when reported.
    pub operating_system: Option<EngineOperatingSystem>,
    /// Engine architecture when reported.
    pub architecture: Option<EngineArchitecture>,
    /// Storage driver name when reported. Driver-specific paths are excluded.
    pub storage_driver: Option<String>,
    /// Logical CPUs available to the engine.
    pub logical_cpus: Option<u64>,
    /// Memory bytes available to the engine.
    pub memory_bytes: Option<u64>,
    /// Total containers reported by the engine.
    pub container_count: Option<u64>,
    /// Running containers reported by the engine.
    pub running_container_count: Option<u64>,
    /// Total images reported by the engine.
    pub image_count: Option<u64>,
}
