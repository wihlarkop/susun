//! Neutral executable engine operation types.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use futures_core::Stream;
use indexmap::IndexMap;
use susun_model::ImageRef;

use crate::{
    ContainerId, LabelKey, LabelValue, NetworkId, ProjectIdentity, ResourceName, ServiceInstanceId,
    VolumeId,
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
