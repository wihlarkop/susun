//! Canonical project type.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    name::{ProjectName, ServiceName},
    resource::{Configs, Networks, Secrets, Volumes},
    service::Service,
};

/// The top-level canonical Compose project.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Project {
    /// Resolved project name.
    pub name: ProjectName,
    /// Ordered map of service name to service definition.
    pub services: IndexMap<ServiceName, Service>,
    /// Top-level network definitions.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub networks: Networks,
    /// Top-level volume definitions.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub volumes: Volumes,
    /// Top-level config definitions.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub configs: Configs,
    /// Top-level secret definitions.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub secrets: Secrets,
}
