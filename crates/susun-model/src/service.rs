//! Canonical service model.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    BuildDefinition, ConfigName, Dependencies, Healthcheck, ImageRef, NetworkAttachment,
    NetworkName, ResourceMount, SecretName, port::CanonicalPort, volume::CanonicalVolume,
};

/// Command or entrypoint in canonical form.
///
/// Compose allows both a string (shell form) and a sequence (exec form).
/// Absence is represented by the enclosing `Option` in [`Service`], not here.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Command {
    /// Shell form: passed to `/bin/sh -c` at runtime.
    Shell(String),
    /// Exec form: executed directly without a shell.
    Exec(Vec<String>),
}

/// A parsed and normalised Compose service definition.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Service {
    /// Container image reference, if specified.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub image: Option<ImageRef>,
    /// Service build definition, if specified.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub build: Option<BuildDefinition>,
    /// Command to run (overrides the image's `CMD`).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub command: Option<Command>,
    /// Entrypoint (overrides the image's `ENTRYPOINT`).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub entrypoint: Option<Command>,
    /// Environment variables; value is `None` when the key inherits from the runtime.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub environment: IndexMap<String, Option<String>>,
    /// Labels attached to the service container.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub labels: IndexMap<String, String>,
    /// Port mappings.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub ports: Vec<CanonicalPort>,
    /// Volume mounts.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub volumes: Vec<CanonicalVolume>,
    /// Service dependencies.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub depends_on: Dependencies,
    /// Networks attached to the service.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub networks: IndexMap<NetworkName, NetworkAttachment>,
    /// Config mounts referenced by this service.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub configs: Vec<ResourceMount<ConfigName>>,
    /// Secret mounts referenced by this service.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub secrets: Vec<ResourceMount<SecretName>>,
    /// Healthcheck configuration.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub healthcheck: Option<Healthcheck>,
    /// Restart policy as written in Compose.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub restart: Option<String>,
    /// Profiles that activate this service.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub profiles: Vec<String>,
}
