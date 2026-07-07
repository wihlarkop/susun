//! Neutral engine contracts for Susun planning.
//!
//! This crate contains daemon-independent value types shared by planners and
//! future runtime adapters. It intentionally has no Docker client dependency.

pub mod capability;
pub mod doctor;
pub mod engine;
pub mod error;
pub mod identity;
pub mod operation;
pub mod profile;
pub mod resource;
pub mod snapshot;

pub use capability::{EngineApiVersion, EngineCapabilities, MountType, SupportLevel};
pub use doctor::{RuntimeDoctorReport, RuntimeDoctorStatus};
pub use engine::{BoxEngineFuture, ContainerEngine};
pub use error::{
    EngineConnectionError, EngineError, EngineOperation, InvalidEngineEndpoint, Platform,
    RedactedEndpoint, ResourceIdentity, TlsConfigurationError,
};
pub use identity::{
    IdentityError, NetworkIdentity, ProjectIdentity, ProjectInstanceId, ReplicaIndex,
    ServiceInstanceId, VolumeIdentity,
};
pub use operation::{
    ActionProgress, BoxByteStream, BoxEventStream, BoxExecStream, BoxLogStream,
    ClientIdentityFiles, ContainerRef, CopyFromContainerRequest, CopyToContainerRequest,
    CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest, EngineArchitecture,
    EngineEndpoint, EngineEndpointKind, EngineEvent, EngineImageRef, EngineOperatingSystem,
    EngineProbe, EngineVersion, EventsRequest, ExecRequest, LogEvent, LogSource, LogsRequest,
    MaterializedResourceMount, NetworkRef, PortRequest, ProgressSink, PruneReport, PruneRequest,
    PruneScope, PublishedPortBinding, PullImageRequest, PullPolicy, RemoveContainerOptions,
    StopContainerRequest, TcpEndpoint, TlsConfiguration, VolumeRef, WaitContainerRequest,
    WaitContainerResult,
};
pub use profile::{
    EngineConnectionDisplayName, EngineConnectionProfile, EngineConnectionProfileError,
    EngineConnectionProfileId, EngineConnectionProfileSet,
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
