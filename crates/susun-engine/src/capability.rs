//! Neutral engine capability contracts.

use indexmap::IndexSet;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Engine API version string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EngineApiVersion(String);

impl EngineApiVersion {
    /// Creates an API version value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the API version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Capability support state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SupportLevel {
    /// Capability is supported.
    Supported,
    /// Capability is supported for a documented subset.
    SupportedSubset,
    /// Capability is available but not considered stable.
    Experimental,
    /// Capability is known to be unsupported.
    Unsupported,
    /// Capability support is unknown.
    #[default]
    Unknown,
}

impl SupportLevel {
    /// Returns true only when the capability is definitely usable.
    pub fn is_supported(self) -> bool {
        matches!(self, Self::Supported | Self::SupportedSubset)
    }
}

/// Mount types understood by the neutral planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum MountType {
    /// Named engine volume.
    Volume,
    /// Host bind mount.
    Bind,
    /// Anonymous runtime volume.
    Anonymous,
}

/// Neutral capabilities reported by an engine adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EngineCapabilities {
    /// Optional engine API version.
    pub api_version: Option<EngineApiVersion>,
    /// Healthcheck support.
    pub supports_health: SupportLevel,
    /// Named volume lifecycle support.
    pub supports_named_volumes: SupportLevel,
    /// Network alias support.
    pub supports_network_aliases: SupportLevel,
    /// Supported mount types.
    pub supports_mount_types: IndexSet<MountType>,
    /// Log-follow capability.
    pub supports_log_follow: SupportLevel,
    /// Image build capability.
    pub supports_build: SupportLevel,
    /// Engine-wide container inventory capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_container_inventory: SupportLevel,
    /// Engine-wide image inventory capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_image_inventory: SupportLevel,
    /// Display-safe engine and system information capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_engine_information: SupportLevel,
    /// Image removal and tagging capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_image_management: SupportLevel,
    /// Registry image pull capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_registry_pull: SupportLevel,
    /// Registry image push capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_registry_push: SupportLevel,
    /// Build-cache inventory and cleanup capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_build_cache: SupportLevel,
    /// Non-destructive cleanup preview capability.
    #[cfg_attr(feature = "serde", serde(default))]
    pub supports_cleanup_preview: SupportLevel,
    /// Optional maximum runtime container-name length.
    pub max_container_name_length: Option<usize>,
}

impl EngineCapabilities {
    /// Returns conservative capabilities for daemon-free Phase 2 planning.
    pub fn conservative() -> Self {
        Self {
            api_version: None,
            supports_health: SupportLevel::Unknown,
            supports_named_volumes: SupportLevel::Unknown,
            supports_network_aliases: SupportLevel::Unknown,
            supports_mount_types: IndexSet::new(),
            supports_log_follow: SupportLevel::Unknown,
            supports_build: SupportLevel::Unsupported,
            supports_container_inventory: SupportLevel::Unknown,
            supports_image_inventory: SupportLevel::Unknown,
            supports_engine_information: SupportLevel::Unknown,
            supports_image_management: SupportLevel::Unsupported,
            supports_registry_pull: SupportLevel::Unknown,
            supports_registry_push: SupportLevel::Unsupported,
            supports_build_cache: SupportLevel::Unsupported,
            supports_cleanup_preview: SupportLevel::Unsupported,
            max_container_name_length: None,
        }
    }

    /// Returns capabilities suitable for local planner examples.
    pub fn permissive_local() -> Self {
        Self {
            api_version: None,
            supports_health: SupportLevel::Supported,
            supports_named_volumes: SupportLevel::Supported,
            supports_network_aliases: SupportLevel::Supported,
            supports_mount_types: IndexSet::from_iter([
                MountType::Volume,
                MountType::Bind,
                MountType::Anonymous,
            ]),
            supports_log_follow: SupportLevel::Supported,
            supports_build: SupportLevel::Unsupported,
            supports_container_inventory: SupportLevel::Unknown,
            supports_image_inventory: SupportLevel::Unknown,
            supports_engine_information: SupportLevel::Unknown,
            supports_image_management: SupportLevel::Unsupported,
            supports_registry_pull: SupportLevel::Supported,
            supports_registry_push: SupportLevel::Unsupported,
            supports_build_cache: SupportLevel::Unsupported,
            supports_cleanup_preview: SupportLevel::Unsupported,
            max_container_name_length: Some(255),
        }
    }

    /// Returns whether the mount type is definitely supported.
    pub fn supports_mount(&self, mount_type: MountType) -> bool {
        self.supports_mount_types.contains(&mount_type)
    }
}

impl Default for EngineCapabilities {
    fn default() -> Self {
        Self::conservative()
    }
}
