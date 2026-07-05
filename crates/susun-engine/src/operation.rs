//! Neutral executable engine operation types.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc, time::Duration};

use futures_core::Stream;
use indexmap::IndexMap;
use susun_model::{
    Command, Healthcheck, ImageRef, NetworkAttachment, port::CanonicalPort, volume::CanonicalVolume,
};

use crate::{
    ContainerId, ImageId, LabelKey, LabelValue, NetworkId, ProjectIdentity, ResourceName,
    ServiceInstanceId, VolumeId,
};

/// Boxed progress future.
pub type ProgressFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Receives non-blocking operation progress.
#[derive(Clone)]
pub struct ProgressSink {
    handler: Arc<dyn Fn(ActionProgress) -> ProgressFuture + Send + Sync>,
}

impl ProgressSink {
    /// Creates a progress sink from an async callback.
    pub fn new(handler: impl Fn(ActionProgress) -> ProgressFuture + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Creates a progress sink that drops all events.
    pub fn discard() -> Self {
        Self::new(|_| Box::pin(async {}))
    }

    /// Emits one progress event.
    pub async fn emit(&self, progress: ActionProgress) {
        (self.handler)(progress).await;
    }
}

impl std::fmt::Debug for ProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressSink").finish_non_exhaustive()
    }
}

/// Engine operation progress.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionProgress {
    /// Stable progress stage.
    pub stage: String,
    /// Optional current units.
    pub current: Option<u64>,
    /// Optional total units.
    pub total: Option<u64>,
    /// Redacted message.
    pub message: Option<String>,
}

/// Docker endpoint selected for an adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EngineEndpoint {
    /// Use local engine discovery.
    Local,
    /// Unix domain socket path.
    UnixSocket(String),
    /// Windows named pipe path.
    WindowsNamedPipe(String),
    /// TCP endpoint. User info is not allowed.
    Tcp {
        /// Host and optional port without credentials.
        host: String,
        /// Whether TLS is enabled.
        tls: bool,
    },
}

impl EngineEndpoint {
    /// Returns a display-safe endpoint.
    pub fn redacted(&self) -> String {
        match self {
            Self::Local => "local".to_owned(),
            Self::UnixSocket(_) => "unix://<local-socket>".to_owned(),
            Self::WindowsNamedPipe(_) => "npipe://<local-pipe>".to_owned(),
            Self::Tcp { host, tls } => {
                let scheme = if *tls { "https" } else { "http" };
                format!("{scheme}://{}", redact_authority(host))
            }
        }
    }
}

fn redact_authority(value: &str) -> String {
    value
        .rsplit('@')
        .next()
        .map(str::to_owned)
        .unwrap_or_else(|| "<redacted>".to_owned())
}

/// Image acquisition policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PullPolicy {
    /// Pull only when the image is missing.
    Missing,
    /// Always pull.
    Always,
}

/// Pull-image request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PullImageRequest {
    /// Image to pull.
    pub image: ImageRef,
    /// Pull policy.
    pub policy: PullPolicy,
}

/// Image reference returned by an engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContainerRef {
    /// Container ID.
    pub id: ContainerId,
}

/// Network reference returned by an engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetworkRef {
    /// Network ID.
    pub id: NetworkId,
}

/// Volume reference returned by an engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeRef {
    /// Volume ID.
    pub id: VolumeId,
}

/// Image reference returned by an engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineImageRef {
    /// Image reference or ID.
    pub reference: String,
}

/// Create-network request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateNetworkRequest {
    /// Project identity.
    pub project: ProjectIdentity,
    /// Runtime name.
    pub name: ResourceName,
    /// Labels to apply.
    pub labels: IndexMap<LabelKey, LabelValue>,
}

/// Create-volume request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateVolumeRequest {
    /// Project identity.
    pub project: ProjectIdentity,
    /// Runtime name.
    pub name: ResourceName,
    /// Labels to apply.
    pub labels: IndexMap<LabelKey, LabelValue>,
}

/// File-backed config or secret materialized into a container.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaterializedResourceMount {
    /// Host-side file path. Contents are never serialized by Susun.
    pub source: PathBuf,
    /// Container target path.
    pub target: String,
    /// Requested uid for the mounted file.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub uid: Option<String>,
    /// Requested gid for the mounted file.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub gid: Option<String>,
    /// Requested file mode.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub mode: Option<String>,
    /// Whether this mount contains secret material.
    pub secret: bool,
}

/// Create-container request for the Phase 3 supported subset.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateContainerRequest {
    /// Project identity.
    pub project: ProjectIdentity,
    /// Service instance identity.
    pub service: ServiceInstanceId,
    /// Runtime name.
    pub name: ResourceName,
    /// Optional image.
    pub image: Option<ImageRef>,
    /// Command override.
    pub command: Option<Command>,
    /// Entrypoint override.
    pub entrypoint: Option<Command>,
    /// Container environment values.
    pub environment: IndexMap<String, Option<String>>,
    /// User-defined container labels.
    pub container_labels: IndexMap<String, String>,
    /// Port mappings.
    pub ports: Vec<CanonicalPort>,
    /// Volume mounts.
    pub volumes: Vec<CanonicalVolume>,
    /// File-backed config mounts.
    pub configs: Vec<MaterializedResourceMount>,
    /// File-backed secret mounts.
    pub secrets: Vec<MaterializedResourceMount>,
    /// Network attachments.
    pub networks: IndexMap<ResourceName, NetworkAttachment>,
    /// Healthcheck configuration.
    pub healthcheck: Option<Healthcheck>,
    /// Restart policy.
    pub restart: Option<String>,
    /// Labels to apply.
    pub labels: IndexMap<LabelKey, LabelValue>,
}

/// Stop-container request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StopContainerRequest {
    /// Container reference.
    pub container: ContainerRef,
    /// Stop timeout.
    pub timeout: Duration,
}

/// Wait-container request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaitContainerRequest {
    /// Container reference.
    pub container: ContainerRef,
}

/// Wait-container result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaitContainerResult {
    /// Process exit code.
    pub exit_code: i64,
}

/// Project-scoped event stream request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventsRequest {
    /// Project identity.
    pub project: ProjectIdentity,
}

/// Neutral project event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineEvent {
    /// Resource kind that emitted the event.
    pub kind: String,
    /// Event action.
    pub action: String,
    /// Adapter resource identifier.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub resource_id: Option<String>,
    /// Safe, redacted event attributes.
    pub attributes: IndexMap<String, String>,
    /// Event timestamp in seconds when supplied by the engine.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub time: Option<i64>,
    /// Event timestamp in nanoseconds when supplied by the engine.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub time_nano: Option<i64>,
}

/// Remove-container options.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemoveContainerOptions {
    /// Remove anonymous volumes.
    pub remove_anonymous_volumes: bool,
    /// Force remove running containers.
    pub force: bool,
}

/// Logs request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogsRequest {
    /// Container reference.
    pub container: ContainerRef,
    /// Follow log stream.
    pub follow: bool,
    /// Include timestamps.
    pub timestamps: bool,
    /// Tail line count.
    pub tail: Option<usize>,
}

/// Exec request for a running container.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecRequest {
    /// Container reference.
    pub container: ContainerRef,
    /// Command and arguments.
    pub command: Vec<String>,
    /// Allocate a pseudo-TTY.
    pub tty: bool,
    /// Attach stdin.
    pub stdin: bool,
    /// Optional user.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub user: Option<String>,
    /// Optional working directory.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub working_dir: Option<String>,
}

/// Exec output stream.
pub type BoxExecStream =
    Pin<Box<dyn Stream<Item = Result<LogEvent, crate::EngineError>> + Send + 'static>>;

/// Boxed neutral engine event stream.
pub type BoxEventStream =
    Pin<Box<dyn Stream<Item = Result<EngineEvent, crate::EngineError>> + Send + 'static>>;

/// Copy archive request from a container path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyFromContainerRequest {
    /// Container reference.
    pub container: ContainerRef,
    /// Container-side source path.
    pub path: String,
}

/// Copy archive request to a container directory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyToContainerRequest {
    /// Container reference.
    pub container: ContainerRef,
    /// Container-side destination directory.
    pub path: String,
    /// Tar archive bytes to extract into `path`.
    pub archive: Vec<u8>,
}

/// Port query request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortRequest {
    /// Container reference.
    pub container: ContainerRef,
    /// Optional container-side port filter.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub private_port: Option<u16>,
    /// Optional protocol filter.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub protocol: Option<String>,
}

/// Published port binding returned by an engine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PublishedPortBinding {
    /// Container-side port.
    pub private_port: u16,
    /// Transport protocol.
    pub protocol: String,
    /// Host IP when supplied by the engine.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub host_ip: Option<String>,
    /// Host-side published port.
    pub host_port: String,
}

/// Boxed neutral byte stream.
pub type BoxByteStream =
    Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::EngineError>> + Send + 'static>>;

/// Log event source stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum LogSource {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
    /// Unknown stream.
    Unknown,
}

/// Neutral log event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogEvent {
    /// Log source.
    pub source: LogSource,
    /// Redacted line bytes as UTF-8 lossily decoded text.
    pub line: String,
}

/// Boxed neutral log stream.
pub type BoxLogStream =
    Pin<Box<dyn Stream<Item = Result<LogEvent, crate::EngineError>> + Send + 'static>>;

/// Resource kinds a prune operation can target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PruneScope {
    /// Stopped containers.
    Containers,
    /// Unused networks.
    Networks,
    /// Unused volumes.
    Volumes,
    /// Unused (dangling) images.
    Images,
}

/// System-wide prune request. Unlike every other operation in this crate,
/// this is NOT scoped to a single project — it can affect resources
/// belonging to any project or tool on the host engine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PruneRequest {
    /// Which resource kinds to prune.
    pub scopes: Vec<PruneScope>,
    /// When `scopes` includes `Images`, also remove unused images that
    /// still carry a tag, not just dangling/untagged ones. A plain prune
    /// (the default, `false`) only removes dangling images — matching
    /// `docker image prune` without `--all` — because Docker never removes
    /// an image that's in use by any container regardless of this flag.
    pub all_images: bool,
}

/// Result of a system-wide prune.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PruneReport {
    /// Removed container IDs.
    pub containers_removed: Vec<ContainerId>,
    /// Removed network IDs.
    pub networks_removed: Vec<NetworkId>,
    /// Removed volume IDs.
    pub volumes_removed: Vec<VolumeId>,
    /// Removed image IDs.
    pub images_removed: Vec<ImageId>,
    /// Total disk space reclaimed, in bytes.
    pub space_reclaimed_bytes: u64,
}
