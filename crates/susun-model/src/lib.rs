//! Canonical domain model for `susun`.
//!
//! This crate has no Susun workspace dependencies. Enable the `serde` feature
//! for JSON serialization of the canonical types.

pub mod name;
pub mod port;
pub mod project;
pub mod service;
pub mod volume;

pub use name::{ImageRef, ProjectName, ServiceName};
pub use port::{CanonicalPort, Protocol, PublishedPort};
pub use project::Project;
pub use service::{Command, Service};
pub use volume::{CanonicalVolume, VolumeKind};
