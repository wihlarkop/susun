//! Test helpers for Susun planning.
//!
//! This crate is intended for tests and compatibility fixtures only. Production
//! crates must not depend on it.

pub mod assertions;
pub mod engine_contract;
pub mod project;
pub mod snapshot;

pub use assertions::PlanAssert;
pub use engine_contract::{
    ContractProject, assert_basic_engine_contract, assert_resource_lifecycle_contract,
};
pub use project::ProjectBuilder;
pub use snapshot::{FakeCapabilities, SnapshotBuilder, SnapshotBuilderError};
