//! Pure convergence planning domain for Susun.
//!
//! This crate owns daemon-independent reconciliation concepts: desired and
//! observed deployments, ownership indexing, convergence policies, and
//! explainable decisions. It must not depend on Docker adapters.

pub mod decision;
pub mod deployment;
pub mod error;
pub mod policy;

pub use decision::{
    ChangedField, ConvergenceDecision, ConvergenceDecisionKind, DecisionReason, InstanceDifference,
    RuntimeDrift,
};
pub use deployment::{
    DesiredDeployment, DesiredReplicaCount, ObservedDeployment, OwnershipConflict, OwnershipIndex,
};
pub use error::ConvergenceError;
pub use policy::{
    AnonymousVolumePolicy, ConvergencePolicy, DependencyRecreatePolicy, ImageChangePolicy,
    OrphanPolicy, RecreatePolicy, ReplacementStrategy,
};
