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
pub mod image;
pub mod impact;
pub mod orphan;
pub mod ownership;
pub mod plan;
pub mod policy;
pub mod scale;
pub mod volume;

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
pub use image::{DesiredImageIdentity, ImageDifference, classify_image_difference};
pub use impact::{DependencyImpact, DependencyWait, propagate_dependency_impact};
pub use orphan::{
    OrphanDisposition, OrphanResource, classify_orphan_containers, classify_orphan_networks,
    classify_orphan_volumes,
};
pub use ownership::{OwnershipConflict, OwnershipIndex};
pub use plan::{
    ConvergencePlanFragment, ReplacementInput, plan_noop_or_start, plan_replacement, plan_scale,
};
pub use policy::{
    AnonymousVolumePolicy, ConvergencePolicy, DependencyRecreatePolicy, ImageChangePolicy,
    OrphanPolicy, RecreatePolicy, ReplacementStrategy,
};
pub use scale::{ScaleDelta, classify_scale_delta, expand_desired_replicas};
pub use volume::{VolumeDisposition, classify_volume_for_replacement};
