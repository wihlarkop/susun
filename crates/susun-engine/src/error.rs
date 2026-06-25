//! Neutral engine error hierarchy.

use std::{error::Error as StdError, fmt, io};

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
    /// Wait for a condition.
    Wait,
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
            Self::Wait => "wait",
        })
    }
}

/// Engine connection errors with redacted endpoint details.
#[derive(Debug, thiserror::Error)]
pub enum EngineConnectionError {
    /// Endpoint could not be reached.
    #[error("engine endpoint unavailable: {endpoint}")]
    EndpointUnavailable {
        /// Redacted endpoint.
        endpoint: String,
        /// Underlying I/O error.
        source: io::Error,
    },
    /// TLS configuration failed.
    #[error("engine TLS configuration failed: {detail}")]
    TlsConfiguration {
        /// Redacted detail.
        detail: String,
    },
    /// API negotiation failed.
    #[error("engine API negotiation failed: {detail}")]
    ApiNegotiation {
        /// Redacted detail.
        detail: String,
    },
    /// Authentication failed.
    #[error("engine authentication failed: {detail}")]
    Authentication {
        /// Redacted detail.
        detail: String,
    },
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
