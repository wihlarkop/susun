//! Neutral engine contracts for Susun planning.
//!
//! This crate contains daemon-independent value types shared by planners and
//! future runtime adapters. It intentionally has no Docker client dependency.

pub mod capability;
pub mod engine;
pub mod error;
pub mod identity;
pub mod operation;
pub mod resource;
pub mod snapshot;

pub use capability::{EngineApiVersion, EngineCapabilities, MountType, SupportLevel};
pub use engine::{BoxEngineFuture, ContainerEngine};
pub use error::{EngineConnectionError, EngineError, EngineOperation, ResourceIdentity};
pub use identity::{
    IdentityError, NetworkIdentity, ProjectIdentity, ProjectInstanceId, ReplicaIndex,
    ServiceInstanceId, VolumeIdentity,
};
pub use operation::{
    ActionProgress, BoxByteStream, BoxEventStream, BoxExecStream, BoxLogStream, ContainerRef,
    CopyFromContainerRequest, CopyToContainerRequest, CreateContainerRequest, CreateNetworkRequest,
    CreateVolumeRequest, EngineEndpoint, EngineEvent, EngineImageRef, EventsRequest, ExecRequest,
    LogEvent, LogSource, LogsRequest, MaterializedResourceMount, NetworkRef, PortRequest,
    ProgressSink, PruneReport, PruneRequest, PruneScope, PublishedPortBinding, PullImageRequest,
    PullPolicy, RemoveContainerOptions, StopContainerRequest, VolumeRef, WaitContainerRequest,
    WaitContainerResult,
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
