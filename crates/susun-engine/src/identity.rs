//! Stable identities for planned engine resources.

use std::{
    fmt,
    path::{Component, Path},
};

use susun_model::{NetworkName, ProjectName, ServiceName, VolumeName};
use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Error returned when an identity value is invalid.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// The provided identity string was empty after trimming whitespace.
    #[error("{kind} must not be empty")]
    Empty {
        /// Name of the rejected identity kind.
        kind: &'static str,
    },
}

/// Logical identity for one managed project instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProjectIdentity {
    /// Compose project name.
    pub name: ProjectName,
    /// Opaque instance ID that separates same-name projects in different roots.
    pub working_set: ProjectInstanceId,
}

impl ProjectIdentity {
    /// Creates a project identity from a name and explicit working set.
    pub fn new(name: ProjectName, working_set: ProjectInstanceId) -> Self {
        Self { name, working_set }
    }
}

/// Opaque stable identifier for a concrete project working set.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ProjectInstanceId(String);

impl ProjectInstanceId {
    /// Creates an explicit project instance ID.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        Self::from_string(value.into(), "project instance ID")
    }

    /// Derives an opaque deterministic ID from project name and directory.
    ///
    /// The source directory contributes to the hash but is not retained, so
    /// formatted identity values do not leak filesystem paths.
    pub fn derive(project_name: &ProjectName, project_directory: impl AsRef<Path>) -> Self {
        let normalized_path = normalize_path(project_directory.as_ref());
        let mut hash = StableHash::new();
        hash.write(project_name.as_str().as_bytes());
        hash.write(&[0]);
        hash.write(normalized_path.as_bytes());
        Self(format!("susun:{:016x}", hash.finish()))
    }

    /// Returns the opaque ID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_string(value: String, kind: &'static str) -> Result<Self, IdentityError> {
        if value.trim().is_empty() {
            return Err(IdentityError::Empty { kind });
        }

        Ok(Self(value))
    }
}

impl fmt::Debug for ProjectInstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProjectInstanceId")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for ProjectInstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ProjectInstanceId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Identity for one service replica.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ServiceInstanceId {
    /// Owning project instance.
    pub project: ProjectInstanceId,
    /// Service key in the canonical project.
    pub service: ServiceName,
    /// Replica index for this service.
    pub replica: ReplicaIndex,
}

impl ServiceInstanceId {
    /// Creates a service instance identity.
    pub fn new(project: ProjectInstanceId, service: ServiceName, replica: ReplicaIndex) -> Self {
        Self {
            project,
            service,
            replica,
        }
    }
}

/// Identity for a declared project network.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NetworkIdentity {
    /// Owning project instance.
    pub project: ProjectInstanceId,
    /// Network key in the canonical project.
    pub network: NetworkName,
}

impl NetworkIdentity {
    /// Creates a network identity.
    pub fn new(project: ProjectInstanceId, network: NetworkName) -> Self {
        Self { project, network }
    }
}

/// Identity for a declared project volume.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VolumeIdentity {
    /// Owning project instance.
    pub project: ProjectInstanceId,
    /// Volume key in the canonical project.
    pub volume: VolumeName,
}

impl VolumeIdentity {
    /// Creates a volume identity.
    pub fn new(project: ProjectInstanceId, volume: VolumeName) -> Self {
        Self { project, volume }
    }
}

/// Zero-based service replica index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ReplicaIndex(u32);

impl ReplicaIndex {
    /// The first replica for a service.
    pub const FIRST: Self = Self(0);

    /// Creates a replica index.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the zero-based index.
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns the one-based ordinal used in runtime names.
    pub fn ordinal(self) -> u32 {
        self.0 + 1
    }
}

impl Default for ReplicaIndex {
    fn default() -> Self {
        Self::FIRST
    }
}

impl fmt::Display for ReplicaIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn normalize_path(path: &Path) -> String {
    let mut parts = Vec::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_string_lossy().to_lowercase())
            }
            Component::RootDir => parts.push(String::new()),
            Component::CurDir => {}
            Component::ParentDir => parts.push("..".to_owned()),
            Component::Normal(part) => parts.push(part.to_string_lossy().to_lowercase()),
        }
    }

    parts.join("/")
}

struct StableHash(u64);

impl StableHash {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(self) -> u64 {
        self.0
    }
}
