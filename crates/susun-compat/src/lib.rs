//! Compatibility harness contracts for Susun.
//!
//! This crate holds neutral configuration for differential checks against an
//! external Docker Compose oracle. It intentionally does not execute Docker on
//! its own yet; later Phase 5 tasks can build runners, reports, and release
//! artifacts on top of these contracts.

pub mod error;
pub mod harness;
pub mod matrix;
pub mod oracle;

pub use error::CompatibilityError;
pub use harness::{CompatibilityHarness, FixtureRunPlan, OracleInvocation};
pub use matrix::{
    CapabilityMatrix, CapabilityMatrixSchemaVersion, FeatureCapability, FeatureSupport,
    matrix_for_current_phase,
};
pub use oracle::{
    ComposeReference, FixtureConfig, OracleCommand, OracleConfig, OracleOperation,
    OracleSchemaVersion,
};
