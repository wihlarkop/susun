//! Neutral engine contracts for Susun planning.
//!
//! This crate contains daemon-independent value types shared by planners and
//! future runtime adapters. It intentionally has no Docker client dependency.

pub mod capability;
pub mod identity;
pub mod resource;
pub mod snapshot;

pub use capability::{EngineApiVersion, EngineCapabilities, MountType, SupportLevel};
pub use identity::{
    IdentityError, NetworkIdentity, ProjectIdentity, ProjectInstanceId, ReplicaIndex,
    ServiceInstanceId, VolumeIdentity,
};
pub use resource::{
    ConfigurationFingerprint, ContainerId, ImageId, LabelKey, LabelValue, NetworkId, ResourceName,
    ResourceNameError, VolumeId,
};
pub use snapshot::{
    ContainerState, EngineSnapshot, HealthState, ObservedContainer, ObservedImage,
    ObservedImageRef, ObservedNetwork, ObservedVolume, SnapshotCompleteness, SnapshotField,
    StableEngineSnapshot,
};
