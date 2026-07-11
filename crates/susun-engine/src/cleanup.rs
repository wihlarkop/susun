//! Non-destructive cleanup inventory and preview contracts.

use crate::{PruneRequest, PruneScope, SupportLevel};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Cleanup preview schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CleanupPreviewSchemaVersion {
    /// Breaking schema generation.
    pub major: u16,
    /// Additive schema generation.
    pub minor: u16,
}
impl CleanupPreviewSchemaVersion {
    /// Current schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Confidence attached to a reclaim estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ReclaimEstimateKind {
    /// Exact engine-reported value.
    Exact,
    /// Conservative lower bound.
    LowerBound,
    /// Engine cannot estimate without pruning.
    Unavailable,
}

/// Preview for one resource scope.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CleanupScopePreview {
    /// Resource scope.
    pub scope: PruneScope,
    /// Preview support level.
    pub support: SupportLevel,
    /// Candidate count when reported.
    pub candidate_count: Option<u64>,
    /// Reclaimable bytes when reported.
    pub reclaimable_bytes: Option<u64>,
    /// Accuracy of the reclaim estimate.
    pub estimate_kind: ReclaimEstimateKind,
}

/// Display-safe, non-destructive cleanup preview.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CleanupPreview {
    /// Serialized contract version.
    pub schema_version: CleanupPreviewSchemaVersion,
    /// Observation time as seconds since Unix epoch.
    pub observed_at_epoch_seconds: u64,
    /// Requested prune policy the preview describes.
    pub request: PruneRequest,
    /// Deterministically ordered scope previews.
    pub scopes: Vec<CleanupScopePreview>,
}
