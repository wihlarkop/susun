//! Raw parsed representation of a Compose file, produced by `susun-loader`.
//!
//! These types form the boundary between the parser adapter and the
//! normalizer. No parser-vendor types appear here.

/// Raw `build` types.
pub mod build;
/// Raw `command` / `entrypoint` types.
pub mod command;
/// Raw `depends_on` types.
pub mod dependency;
/// Raw `environment` / `labels` types.
pub mod environment;
/// Raw `healthcheck` types.
pub mod health;
/// Raw `ports` types.
pub mod port;
/// Raw resource reference types.
pub mod resource;
/// Raw service entry type.
pub mod service;
/// Raw `volumes` types.
pub mod volume;

pub use build::RawBuildDefinition;
pub use command::RawStringOrList;
pub use dependency::{RawDependencies, RawDependency};
pub use environment::RawMapping;
pub use health::RawHealthcheck;
pub use port::{RawPortEntry, RawPortLong, RawPortShort};
pub use resource::{
    RawNetworkAttachment, RawResourceDefinition, RawResourceMount, RawResources, RawServiceNetworks,
};
pub use service::ParsedService;
pub use volume::{RawVolumeLong, RawVolumeMount, RawVolumeShort};

use indexmap::IndexMap;
use susun_source::Spanned;

/// Raw parsed representation of a single Compose file.
pub struct ParsedProject {
    /// The top-level `name:` field, if present.
    pub name: Option<Spanned<String>>,
    /// Services declared under `services:`, keyed by service name.
    pub services: IndexMap<String, Spanned<ParsedService>>,
    /// Top-level networks.
    pub networks: RawResources,
    /// Top-level volumes.
    pub volumes: RawResources,
    /// Top-level configs.
    pub configs: RawResources,
    /// Top-level secrets.
    pub secrets: RawResources,
}

/// Merged representation passed to the normalizer.
pub struct MergeProject {
    /// The top-level `name:` field, if present.
    pub name: Option<Spanned<String>>,
    /// Services, keyed by service name.
    pub services: IndexMap<String, Spanned<ParsedService>>,
    /// Top-level networks.
    pub networks: RawResources,
    /// Top-level volumes.
    pub volumes: RawResources,
    /// Top-level configs.
    pub configs: RawResources,
    /// Top-level secrets.
    pub secrets: RawResources,
}

impl From<ParsedProject> for MergeProject {
    fn from(p: ParsedProject) -> Self {
        MergeProject {
            name: p.name,
            services: p.services,
            networks: p.networks,
            volumes: p.volumes,
            configs: p.configs,
            secrets: p.secrets,
        }
    }
}
