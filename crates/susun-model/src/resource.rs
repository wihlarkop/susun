//! Canonical Compose resource model.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{ConfigName, NetworkName, SecretName, VolumeName};

/// Top-level config, secret, network, or volume definition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ResourceDefinition {
    /// Whether the resource is external.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub external: bool,
    /// Runtime resource name override.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    /// File backing for config/secret resources.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub file: Option<String>,
}

/// Service network attachment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NetworkAttachment {
    /// Network aliases.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub aliases: Vec<String>,
}

/// Service config or secret mount.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ResourceMount<N> {
    /// Referenced top-level resource name.
    pub source: N,
    /// Container target path/name.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub target: Option<String>,
    /// Requested uid for the mounted file.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub uid: Option<String>,
    /// Requested gid for the mounted file.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub gid: Option<String>,
    /// Requested file mode.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub mode: Option<String>,
}

/// Project networks.
pub type Networks = IndexMap<NetworkName, ResourceDefinition>;
/// Project volumes.
pub type Volumes = IndexMap<VolumeName, ResourceDefinition>;
/// Project configs.
pub type Configs = IndexMap<ConfigName, ResourceDefinition>;
/// Project secrets.
pub type Secrets = IndexMap<SecretName, ResourceDefinition>;

#[cfg(feature = "serde")]
fn is_false(value: &bool) -> bool {
    !*value
}
