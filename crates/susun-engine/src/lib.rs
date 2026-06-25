//! Neutral engine contracts for Susun planning.
//!
//! This crate contains daemon-independent value types shared by planners and
//! future runtime adapters. It intentionally has no Docker client dependency.

pub mod identity;
pub mod resource;

pub use identity::{
    IdentityError, NetworkIdentity, ProjectIdentity, ProjectInstanceId, ReplicaIndex,
    ServiceInstanceId, VolumeIdentity,
};
pub use resource::{ResourceName, ResourceNameError};
