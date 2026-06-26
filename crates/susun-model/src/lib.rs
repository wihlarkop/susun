//! Canonical domain model for `susun`.
//!
//! This crate has no Susun workspace dependencies. Enable the `serde` feature
//! for JSON serialization of the canonical types.

pub mod build;
pub mod dependency;
pub mod health;
pub mod name;
pub mod port;
pub mod project;
pub mod resource;
pub mod service;
pub mod volume;

pub use build::BuildDefinition;
pub use dependency::{Dependencies, DependencyCondition, ServiceDependency};
pub use health::Healthcheck;
pub use name::{
    ConfigName, ImageRef, NetworkName, ProfileName, ProjectName, SecretName, ServiceName,
    VolumeName,
};
pub use port::{CanonicalPort, Protocol, PublishedPort};
pub use project::Project;
pub use resource::{
    Configs, NetworkAttachment, Networks, ResourceDefinition, ResourceMount, Secrets, Volumes,
};
pub use service::{Command, Service};
pub use volume::{CanonicalVolume, VolumeKind};
