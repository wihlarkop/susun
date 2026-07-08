//! Neutral engine error hierarchy.

use std::{error::Error as StdError, fmt};

use crate::{ContainerId, NetworkId, ResourceName, VolumeId};

/// Boxed source error used at engine boundaries.
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// Engine operation keys used in errors and reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum EngineOperation {
    /// Capability discovery.
    Capabilities,
    /// Snapshot acquisition.
    Snapshot,
    /// Pull image.
    PullImage,
    /// Create network.
    CreateNetwork,
    /// Remove network.
    RemoveNetwork,
    /// Create volume.
    CreateVolume,
    /// Remove volume.
    RemoveVolume,
    /// Create container.
    CreateContainer,
    /// Start container.
    StartContainer,
    /// Stop container.
    StopContainer,
    /// Remove container.
    RemoveContainer,
    /// Stream logs.
    Logs,
    /// Stream engine events.
    Events,
    /// Execute a command in a container.
    Exec,
    /// Copy files to or from a container.
    Copy,
    /// Query published container ports.
    Port,
    /// Wait for a condition.
    Wait,
    /// System-wide prune.
    Prune,
}

impl fmt::Display for EngineOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Capabilities => "capabilities",
            Self::Snapshot => "snapshot",
            Self::PullImage => "pull image",
            Self::CreateNetwork => "create network",
            Self::RemoveNetwork => "remove network",
            Self::CreateVolume => "create volume",
            Self::RemoveVolume => "remove volume",
            Self::CreateContainer => "create container",
            Self::StartContainer => "start container",
            Self::StopContainer => "stop container",
            Self::RemoveContainer => "remove container",
            Self::Logs => "logs",
            Self::Events => "events",
            Self::Exec => "exec",
            Self::Copy => "copy",
            Self::Port => "port",
            Self::Wait => "wait",
            Self::Prune => "prune",
        })
    }
}

/// A target platform, used to explain why an endpoint kind isn't
/// supported here (e.g. a Windows named pipe requested on Linux).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Platform {
    /// Windows.
    Windows,
    /// macOS.
    MacOs,
    /// Linux.
    Linux,
    /// Any other platform.
    Other,
}

impl Platform {
    /// Returns the platform this code is currently compiled/running for.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// A display-safe endpoint, only ever constructed by redacting a real
/// `crate::EngineEndpoint` — there is no path to build one from an
/// arbitrary unredacted string. Serde deserialization accepts only Susun's
/// known redacted endpoint tokens so persisted UI/API payloads cannot inject
/// arbitrary endpoint text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct RedactedEndpoint(String);

impl RedactedEndpoint {
    /// Redacts an endpoint for safe display.
    pub fn new(endpoint: &crate::EngineEndpoint) -> Self {
        Self(endpoint.redacted())
    }

    #[cfg(feature = "serde")]
    fn from_serialized_redacted(value: String) -> Option<Self> {
        matches!(
            value.as_str(),
            "local"
                | "unix://<local-socket>"
                | "npipe://<local-pipe>"
                | "http://<remote-host>"
                | "https://<remote-host>"
        )
        .then_some(Self(value))
    }
}

impl fmt::Display for RedactedEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RedactedEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::from_serialized_redacted(value)
            .ok_or_else(|| serde::de::Error::custom("unsupported redacted engine endpoint"))
    }
}

/// Engine connection errors with redacted endpoint details. The first
/// three variants can occur while constructing a client
/// (`BollardEngine::connect_to`); the latter three normally occur while
/// proving reachability (`BollardEngine::probe`) or during later engine
/// operations.
#[derive(Debug, thiserror::Error)]
pub enum EngineConnectionError {
    /// The endpoint value itself was invalid.
    #[error("invalid engine endpoint: {detail}")]
    InvalidEndpoint {
        /// Redacted detail.
        detail: String,
    },
    /// This endpoint kind isn't supported on the current platform.
    #[error("engine endpoint kind {endpoint_kind:?} is not supported on {platform:?}")]
    UnsupportedEndpoint {
        /// Which endpoint kind was requested.
        endpoint_kind: crate::EngineEndpointKind,
        /// Which platform rejected it.
        platform: Platform,
    },
    /// TLS configuration failed.
    #[error("engine TLS configuration failed: {detail}")]
    TlsConfiguration {
        /// Redacted detail.
        detail: String,
    },
    /// Endpoint could not be reached.
    #[error("engine endpoint unavailable: {endpoint}")]
    EndpointUnavailable {
        /// Redacted endpoint.
        endpoint: RedactedEndpoint,
        /// Underlying error.
        source: BoxError,
    },
    /// API negotiation failed.
    #[error("engine API negotiation failed: {source}")]
    ApiNegotiation {
        /// Underlying error.
        source: BoxError,
    },
    /// Authentication failed.
    #[error("engine authentication failed: {endpoint}")]
    Authentication {
        /// Redacted endpoint.
        endpoint: RedactedEndpoint,
        /// Underlying error.
        source: BoxError,
    },
}

/// A `TcpEndpoint` was constructed with an invalid host or port.
#[derive(Debug, thiserror::Error)]
pub enum InvalidEngineEndpoint {
    /// Host was empty.
    #[error("host must not be empty")]
    EmptyHost,
    /// Host included a URL scheme (e.g. `http://`).
    #[error("host must not include a URL scheme")]
    EmbeddedScheme,
    /// Host included embedded credentials (an `@`).
    #[error("host must not include credentials")]
    EmbeddedCredentials,
    /// Host included a path or query component.
    #[error("host must not include a path or query")]
    EmbeddedPathOrQuery,
    /// Port was zero.
    #[error("port must not be 0")]
    PortZero,
    /// Host used bracket syntax but was not a valid bracketed IPv6 address.
    #[error("malformed bracketed IPv6 host")]
    MalformedIpv6,
    /// Host looks like IPv6 but did not use bracketed authority syntax.
    #[error("IPv6 host must use bracketed authority syntax")]
    UnbracketedIpv6,
}

/// A `ClientIdentityFiles`/`TlsConfiguration` was constructed with an
/// incomplete or invalid combination of fields.
#[derive(Debug, thiserror::Error)]
pub enum TlsConfigurationError {
    /// A client identity requires both a certificate and a private key —
    /// one was supplied without the other, or one was empty.
    #[error("client identity requires both a certificate and a private key")]
    IncompleteClientIdentity,
}

/// Resource identity for typed engine errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum ResourceIdentity {
    /// Runtime resource name.
    Name(ResourceName),
    /// Container ID.
    Container(ContainerId),
    /// Network ID.
    Network(NetworkId),
    /// Volume ID.
    Volume(VolumeId),
    /// Image reference or ID.
    Image(String),
}

impl fmt::Display for ResourceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(value) => write!(f, "name {value}"),
            Self::Container(value) => write!(f, "container {value}"),
            Self::Network(value) => write!(f, "network {value}"),
            Self::Volume(value) => write!(f, "volume {value}"),
            Self::Image(value) => write!(f, "image {value}"),
        }
    }
}

/// Neutral engine error.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Connection-level failure.
    #[error(transparent)]
    Connection(#[from] EngineConnectionError),
    /// API-level failure.
    #[error("engine {operation} failed: {source}")]
    Api {
        /// Operation being performed.
        operation: EngineOperation,
        /// Redacted source error.
        source: BoxError,
    },
    /// Unsupported capability.
    #[error("engine does not support {capability}")]
    Unsupported {
        /// Capability key.
        capability: &'static str,
    },
    /// Resource conflict.
    #[error("engine resource conflict for {resource}: {detail}")]
    Conflict {
        /// Conflicting resource.
        resource: ResourceIdentity,
        /// Redacted detail.
        detail: String,
    },
    /// Resource not found.
    #[error("engine resource not found: {resource}")]
    NotFound {
        /// Missing resource.
        resource: ResourceIdentity,
    },
    /// Authentication failed.
    #[error("engine authentication failed for {registry}")]
    Authentication {
        /// Redacted registry.
        registry: String,
    },
    /// Operation was cancelled.
    #[error("engine operation cancelled")]
    Cancelled,
}

impl EngineError {
    /// Wraps an adapter API error.
    pub fn api(operation: EngineOperation, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Api {
            operation,
            source: Box::new(source),
        }
    }
}
