//! Neutral executable engine operation types.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc, time::Duration};

use futures_core::Stream;
use indexmap::IndexMap;
use susun_model::{
    Command, Healthcheck, ImageRef, NetworkAttachment, port::CanonicalPort, volume::CanonicalVolume,
};

use crate::{
    ContainerId, EngineApiVersion, ImageId, LabelKey, LabelValue, NetworkId, ProjectIdentity,
    ResourceName, ServiceInstanceId, VolumeId,
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
    /// Stable operation kind.
    pub operation: EngineProgressOperation,
    /// Stable progress stage.
    pub stage: String,
    /// Optional current units.
    pub current: Option<u64>,
    /// Optional total units.
    pub total: Option<u64>,
    /// Redacted message.
    pub message: Option<String>,
}

/// Stable engine operation identity attached to progress events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum EngineProgressOperation {
    /// Image pull.
    PullImage,
    /// Image push.
    PushImage,
}

/// Docker-compatible endpoint selected for an adapter. Constructing one
/// never touches the network — connecting and probing are separate,
/// explicit steps (see `BollardEngine::connect_to`/`probe`).
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EngineEndpoint {
    /// Use the platform's conventional local Docker-compatible endpoint.
    /// Does NOT perform multi-runtime discovery — callers that need to
    /// pick between Docker Desktop/Podman/Rancher Desktop/etc. resolve
    /// that themselves and construct an explicit variant below.
    Local,
    /// Connect to this exact Unix socket path.
    UnixSocket(PathBuf),
    /// Connect to this exact Windows named pipe.
    WindowsNamedPipe(Arc<str>),
    /// Connect to this exact TCP endpoint.
    Tcp(TcpEndpoint),
}

/// Fieldless mirror of `EngineEndpoint`'s variants, for error reporting
/// (e.g. `EngineConnectionError::UnsupportedEndpoint`) without needing to
/// carry — or redact — the endpoint's actual contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum EngineEndpointKind {
    /// Mirrors `EngineEndpoint::Local`.
    Local,
    /// Mirrors `EngineEndpoint::UnixSocket`.
    UnixSocket,
    /// Mirrors `EngineEndpoint::WindowsNamedPipe`.
    WindowsNamedPipe,
    /// Mirrors `EngineEndpoint::Tcp`.
    Tcp,
}

impl EngineEndpoint {
    /// Returns a display-safe endpoint.
    pub fn redacted(&self) -> String {
        match self {
            Self::Local => "local".to_owned(),
            Self::UnixSocket(_) => "unix://<local-socket>".to_owned(),
            Self::WindowsNamedPipe(_) => "npipe://<local-pipe>".to_owned(),
            Self::Tcp(endpoint) => {
                let scheme = if endpoint.tls().is_some() {
                    "https"
                } else {
                    "http"
                };
                format!("{scheme}://<remote-host>")
            }
        }
    }

    /// Returns this endpoint's kind, without its contents.
    pub fn kind(&self) -> EngineEndpointKind {
        match self {
            Self::Local => EngineEndpointKind::Local,
            Self::UnixSocket(_) => EngineEndpointKind::UnixSocket,
            Self::WindowsNamedPipe(_) => EngineEndpointKind::WindowsNamedPipe,
            Self::Tcp(_) => EngineEndpointKind::Tcp,
        }
    }
}

impl std::fmt::Debug for EngineEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.redacted())
    }
}

fn validate_host(host: &str) -> Result<(), crate::InvalidEngineEndpoint> {
    if host.is_empty() {
        return Err(crate::InvalidEngineEndpoint::EmptyHost);
    }
    if host.contains("http://") || host.contains("https://") {
        return Err(crate::InvalidEngineEndpoint::EmbeddedScheme);
    }
    if host.contains('@') {
        return Err(crate::InvalidEngineEndpoint::EmbeddedCredentials);
    }
    if host.contains('/') || host.contains('?') {
        return Err(crate::InvalidEngineEndpoint::EmbeddedPathOrQuery);
    }
    let has_open = host.starts_with('[');
    let has_close = host.ends_with(']');
    if has_open != has_close {
        return Err(crate::InvalidEngineEndpoint::MalformedIpv6);
    }
    if has_open && has_close {
        host[1..host.len() - 1]
            .parse::<std::net::Ipv6Addr>()
            .map_err(|_| crate::InvalidEngineEndpoint::MalformedIpv6)?;
    } else if host.contains(':') {
        return Err(crate::InvalidEngineEndpoint::UnbracketedIpv6);
    }
    Ok(())
}

/// TLS client certificate + private key, always constructed as a pair —
/// there is no way to represent one without the other.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ClientIdentityFiles {
    certificate: PathBuf,
    private_key: PathBuf,
}

impl ClientIdentityFiles {
    /// Constructs a client identity. Fails if either path is empty.
    pub fn new(
        certificate: impl Into<PathBuf>,
        private_key: impl Into<PathBuf>,
    ) -> Result<Self, crate::TlsConfigurationError> {
        let certificate = certificate.into();
        let private_key = private_key.into();
        if certificate.as_os_str().is_empty() || private_key.as_os_str().is_empty() {
            return Err(crate::TlsConfigurationError::IncompleteClientIdentity);
        }
        Ok(Self {
            certificate,
            private_key,
        })
    }

    /// Returns the certificate path.
    pub fn certificate(&self) -> &std::path::Path {
        &self.certificate
    }

    /// Returns the private key path.
    pub fn private_key(&self) -> &std::path::Path {
        &self.private_key
    }
}

impl std::fmt::Debug for ClientIdentityFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientIdentityFiles")
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ClientIdentityFiles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct ClientIdentityFilesSerde {
            certificate: PathBuf,
            private_key: PathBuf,
        }

        let value = <ClientIdentityFilesSerde as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(value.certificate, value.private_key).map_err(serde::de::Error::custom)
    }
}

/// TLS configuration for a Docker-compatible `Tcp` endpoint. The current
/// Bollard adapter supports Docker's mutual-TLS file model: a custom CA
/// certificate plus a client certificate/key pair.
///
/// Fields are private. Construct via `new()` plus the `with_*` builder
/// methods, never a struct literal.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TlsConfiguration {
    ca_certificate: Option<PathBuf>,
    client_identity: Option<ClientIdentityFiles>,
    server_name: Option<Arc<str>>,
}

impl TlsConfiguration {
    /// Starts an empty TLS configuration.
    ///
    /// Adapters may validate that the required files are present when a
    /// connection is constructed. For the Bollard adapter, `with_ca_certificate`
    /// and `with_client_identity` are both required.
    pub fn new() -> Self {
        Self {
            ca_certificate: None,
            client_identity: None,
            server_name: None,
        }
    }

    /// Adds a custom CA certificate path.
    pub fn with_ca_certificate(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_certificate = Some(path.into());
        self
    }

    /// Adds a client certificate + key for mutual TLS.
    pub fn with_client_identity(mut self, identity: ClientIdentityFiles) -> Self {
        self.client_identity = Some(identity);
        self
    }

    /// Reserves a TLS server-name override for adapters that support it.
    ///
    /// The current Bollard adapter rejects this field because Bollard 0.21's
    /// SSL constructor does not expose an explicit server-name override.
    pub fn with_server_name(mut self, name: impl Into<Arc<str>>) -> Self {
        self.server_name = Some(name.into());
        self
    }

    /// Returns the custom CA certificate path, if any.
    pub fn ca_certificate(&self) -> Option<&std::path::Path> {
        self.ca_certificate.as_deref()
    }

    /// Returns the client identity, if any.
    pub fn client_identity(&self) -> Option<&ClientIdentityFiles> {
        self.client_identity.as_ref()
    }

    /// Returns the TLS server-name override, if any.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }
}

impl Default for TlsConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TlsConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConfiguration")
            .field("ca_certificate", &self.ca_certificate.is_some())
            .field("client_identity", &self.client_identity.is_some())
            .field("server_name", &self.server_name.is_some())
            .finish()
    }
}

/// A TCP endpoint: host and port, never a loosely-validated URL string.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TcpEndpoint {
    host: Arc<str>,
    port: u16,
    tls: Option<TlsConfiguration>,
}

impl TcpEndpoint {
    /// Constructs a TCP endpoint. Rejects an empty host, a host containing
    /// a URL scheme/credentials/path/query, port 0, or malformed bracketed
    /// IPv6 syntax.
    pub fn new(host: impl Into<Arc<str>>, port: u16) -> Result<Self, crate::InvalidEngineEndpoint> {
        let host = host.into();
        validate_host(&host)?;
        if port == 0 {
            return Err(crate::InvalidEngineEndpoint::PortZero);
        }
        Ok(Self {
            host,
            port,
            tls: None,
        })
    }

    /// Attaches TLS configuration to this endpoint.
    pub fn with_tls(mut self, tls: TlsConfiguration) -> Self {
        self.tls = Some(tls);
        self
    }

    /// Returns the host.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the TLS configuration, if any.
    pub fn tls(&self) -> Option<&TlsConfiguration> {
        self.tls.as_ref()
    }
}

impl std::fmt::Debug for TcpEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpEndpoint")
            .field("host", &"<redacted>")
            .field("port", &self.port)
            .field("tls", &self.tls.is_some())
            .finish()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TcpEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct TcpEndpointSerde {
            host: Arc<str>,
            port: u16,
            #[serde(default)]
            tls: Option<TlsConfiguration>,
        }

        let value = <TcpEndpointSerde as serde::Deserialize>::deserialize(deserializer)?;
        let endpoint = Self::new(value.host, value.port).map_err(serde::de::Error::custom)?;
        Ok(match value.tls {
            Some(tls) => endpoint.with_tls(tls),
            None => endpoint,
        })
    }
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

/// Engine daemon version string (distinct from the API version).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EngineVersion(String);

impl EngineVersion {
    /// Creates an engine version value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the engine version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Operating system the engine daemon reports running on (e.g. `"linux"`,
/// `"windows"`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EngineOperatingSystem(String);

impl EngineOperatingSystem {
    /// Creates an operating-system value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the operating-system string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Architecture the engine daemon reports running on (e.g. `"amd64"`,
/// `"arm64"`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EngineArchitecture(String);

impl EngineArchitecture {
    /// Creates an architecture value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the architecture string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Result of probing a connected engine for liveness and version
/// information. Neutral — not Bollard-specific — even though today only
/// `BollardEngine::probe` fills it in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineProbe {
    /// Negotiated/reported API version.
    pub api_version: Option<EngineApiVersion>,
    /// Engine daemon version.
    pub engine_version: Option<EngineVersion>,
    /// Reported operating system.
    pub operating_system: Option<EngineOperatingSystem>,
    /// Reported architecture.
    pub architecture: Option<EngineArchitecture>,
}
