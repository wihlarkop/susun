//! Runtime naming policies for planned resources.

use susun_engine::{NetworkIdentity, ResourceName, ServiceInstanceId, VolumeIdentity};
use thiserror::Error;

/// Converts stable Susun identities into runtime-visible resource names.
pub trait NamingPolicy: Send + Sync {
    /// Returns the runtime container name for a service instance.
    fn container_name(&self, id: &ServiceInstanceId) -> Result<ResourceName, NamingError>;

    /// Returns the runtime network name for a project network.
    fn network_name(&self, id: &NetworkIdentity) -> Result<ResourceName, NamingError>;

    /// Returns the runtime volume name for a project volume.
    fn volume_name(&self, id: &VolumeIdentity) -> Result<ResourceName, NamingError>;
}

/// Error returned by naming policies.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NamingError {
    /// Name generation produced an empty runtime name.
    #[error("generated {kind} name is empty")]
    Empty {
        /// Resource kind being named.
        kind: &'static str,
    },
    /// The generated name exceeds the configured backend limit.
    #[error("generated {kind} name '{name}' exceeds limit {limit}")]
    TooLong {
        /// Resource kind being named.
        kind: &'static str,
        /// Generated name.
        name: String,
        /// Configured maximum length.
        limit: usize,
    },
}

/// Default Susun runtime naming policy.
#[derive(Debug, Clone, Default)]
pub struct SusunNamingPolicy {
    max_name_length: Option<usize>,
}

impl SusunNamingPolicy {
    /// Creates a default naming policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a policy that refuses names longer than `max_name_length`.
    pub fn with_max_name_length(max_name_length: usize) -> Self {
        Self {
            max_name_length: Some(max_name_length),
        }
    }
}

impl NamingPolicy for SusunNamingPolicy {
    fn container_name(&self, id: &ServiceInstanceId) -> Result<ResourceName, NamingError> {
        runtime_name(
            "container",
            self.max_name_length,
            [
                "susun",
                id.project.as_str(),
                id.service.as_str(),
                &id.replica.ordinal().to_string(),
            ],
            "-",
        )
    }

    fn network_name(&self, id: &NetworkIdentity) -> Result<ResourceName, NamingError> {
        runtime_name(
            "network",
            self.max_name_length,
            ["susun", id.project.as_str(), id.network.as_str()],
            "-",
        )
    }

    fn volume_name(&self, id: &VolumeIdentity) -> Result<ResourceName, NamingError> {
        runtime_name(
            "volume",
            self.max_name_length,
            ["susun", id.project.as_str(), id.volume.as_str()],
            "-",
        )
    }
}

/// Compose-style runtime naming policy.
#[derive(Debug, Clone, Default)]
pub struct ComposeCompatibleNamingPolicy {
    max_name_length: Option<usize>,
}

impl ComposeCompatibleNamingPolicy {
    /// Creates a Compose-compatible naming policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a policy that refuses names longer than `max_name_length`.
    pub fn with_max_name_length(max_name_length: usize) -> Self {
        Self {
            max_name_length: Some(max_name_length),
        }
    }
}

impl NamingPolicy for ComposeCompatibleNamingPolicy {
    fn container_name(&self, id: &ServiceInstanceId) -> Result<ResourceName, NamingError> {
        runtime_name(
            "container",
            self.max_name_length,
            [
                id.project.as_str(),
                id.service.as_str(),
                &id.replica.ordinal().to_string(),
            ],
            "_",
        )
    }

    fn network_name(&self, id: &NetworkIdentity) -> Result<ResourceName, NamingError> {
        runtime_name(
            "network",
            self.max_name_length,
            [id.project.as_str(), id.network.as_str()],
            "_",
        )
    }

    fn volume_name(&self, id: &VolumeIdentity) -> Result<ResourceName, NamingError> {
        runtime_name(
            "volume",
            self.max_name_length,
            [id.project.as_str(), id.volume.as_str()],
            "_",
        )
    }
}

fn runtime_name<'a>(
    kind: &'static str,
    max_name_length: Option<usize>,
    parts: impl IntoIterator<Item = &'a str>,
    separator: &str,
) -> Result<ResourceName, NamingError> {
    let name = parts
        .into_iter()
        .map(normalize_part)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(separator);

    if name.is_empty() {
        return Err(NamingError::Empty { kind });
    }

    if let Some(limit) = max_name_length
        && name.len() > limit
    {
        return Err(NamingError::TooLong { kind, name, limit });
    }

    ResourceName::new(name).map_err(|_| NamingError::Empty { kind })
}

fn normalize_part(part: &str) -> String {
    let mut normalized = String::with_capacity(part.len());
    let mut previous_separator = false;

    for ch in part.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            previous_separator = false;
            ch.to_ascii_lowercase()
        } else if previous_separator {
            continue;
        } else {
            previous_separator = true;
            '-'
        };
        normalized.push(mapped);
    }

    normalized.trim_matches('-').to_owned()
}
