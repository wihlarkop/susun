//! Pure convergence planning domain for Susun.
//!
//! This crate owns daemon-independent reconciliation concepts: desired and
//! observed deployments, ownership indexing, convergence policies, and
//! explainable decisions. It must not depend on Docker adapters.

pub mod decision;
pub mod deployment;
pub mod diagnostics;
pub mod diff;
pub mod error;
pub mod fingerprint;
pub mod ownership;
pub mod policy;

pub use decision::{
    ChangedField, ConvergenceDecision, ConvergenceDecisionKind, DecisionReason, InstanceDifference,
    RuntimeDrift,
};
pub use deployment::{DesiredDeployment, DesiredReplicaCount, ObservedDeployment};
pub use diagnostics::{
    SUS_DIFF_CONFIG_CHANGED, SUS_DIFF_MISSING, SUS_DIFF_UNSUPPORTED_STATE,
    SUS_DIFF_UNSUPPORTED_VERSION, SUS_FP_UNSUPPORTED_VERSION, SUS_OWN_DUPLICATE_CLAIM,
    SUS_OWN_FOREIGN_NAME, convergence_diagnostic_for_difference, fingerprint_version_diagnostic,
    foreign_name_conflict_diagnostic, ownership_conflict_diagnostic,
};
pub use diff::{DesiredInstanceFingerprints, classify_deployment_differences};
pub use error::ConvergenceError;
pub use fingerprint::{
    CanonicalFingerprintInput, FingerprintDigest, FingerprintInput, FingerprintVersion,
    ResolvedImageIdentity, ResolvedResourceNames, RuntimeDefaults, VersionedFingerprint,
    compute_configuration_fingerprint, parse_configuration_fingerprint,
};
pub use ownership::{OwnershipConflict, OwnershipIndex};
pub use policy::{
    AnonymousVolumePolicy, ConvergencePolicy, DependencyRecreatePolicy, ImageChangePolicy,
    OrphanPolicy, RecreatePolicy, ReplacementStrategy,
};
