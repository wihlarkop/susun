//! Engine snapshot fixture builders.

use std::time::SystemTime;

use susun_engine::{
    ContainerId, EngineCapabilities, EngineSnapshot, ImageId, NetworkId, ObservedContainer,
    ObservedImage, ObservedNetwork, ObservedVolume, VolumeId,
};
use thiserror::Error;

/// Error returned by snapshot builders.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SnapshotBuilderError {
    /// A duplicate container ID was inserted.
    #[error("duplicate container id {0}")]
    DuplicateContainer(ContainerId),
    /// A duplicate network ID was inserted.
    #[error("duplicate network id {0}")]
    DuplicateNetwork(NetworkId),
    /// A duplicate volume ID was inserted.
    #[error("duplicate volume id {0}")]
    DuplicateVolume(VolumeId),
    /// A duplicate image ID was inserted.
    #[error("duplicate image id {0}")]
    DuplicateImage(ImageId),
}

/// Builder for neutral engine snapshots.
#[derive(Debug, Clone)]
pub struct SnapshotBuilder {
    snapshot: EngineSnapshot,
}

impl SnapshotBuilder {
    /// Creates a snapshot builder with deterministic UNIX epoch observation time.
    pub fn new() -> Self {
        Self {
            snapshot: EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
        }
    }

    /// Adds an observed container.
    pub fn container(mut self, container: ObservedContainer) -> Result<Self, SnapshotBuilderError> {
        if self.snapshot.containers.contains_key(&container.id) {
            return Err(SnapshotBuilderError::DuplicateContainer(container.id));
        }

        self.snapshot
            .containers
            .insert(container.id.clone(), container);
        Ok(self)
    }

    /// Adds an observed network.
    pub fn network(mut self, network: ObservedNetwork) -> Result<Self, SnapshotBuilderError> {
        if self.snapshot.networks.contains_key(&network.id) {
            return Err(SnapshotBuilderError::DuplicateNetwork(network.id));
        }

        self.snapshot.networks.insert(network.id.clone(), network);
        Ok(self)
    }

    /// Adds an observed volume.
    pub fn volume(mut self, volume: ObservedVolume) -> Result<Self, SnapshotBuilderError> {
        if self.snapshot.volumes.contains_key(&volume.id) {
            return Err(SnapshotBuilderError::DuplicateVolume(volume.id));
        }

        self.snapshot.volumes.insert(volume.id.clone(), volume);
        Ok(self)
    }

    /// Adds an observed image.
    pub fn image(mut self, image: ObservedImage) -> Result<Self, SnapshotBuilderError> {
        if self.snapshot.images.contains_key(&image.id) {
            return Err(SnapshotBuilderError::DuplicateImage(image.id));
        }

        self.snapshot.images.insert(image.id.clone(), image);
        Ok(self)
    }

    /// Builds the snapshot.
    pub fn build(self) -> EngineSnapshot {
        self.snapshot
    }
}

impl Default for SnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Common fake capability sets for planner fixtures.
#[derive(Debug, Clone, Copy)]
pub struct FakeCapabilities;

impl FakeCapabilities {
    /// Conservative unknown/unsupported capability set.
    pub fn conservative() -> EngineCapabilities {
        EngineCapabilities::conservative()
    }

    /// Permissive local capability set for dry-run planner examples.
    pub fn permissive_local() -> EngineCapabilities {
        EngineCapabilities::permissive_local()
    }
}
