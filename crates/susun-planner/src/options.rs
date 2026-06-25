//! Planner policy options.

use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Image acquisition policy for `up` planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ImageAcquisitionPolicy {
    /// Plan pulls for images missing from the snapshot.
    PullIfMissing,
    /// Never plan image pulls.
    NeverPull,
    /// Require images to already be available.
    RequirePresent,
}

/// Existing resource handling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ExistingResourcePolicy {
    /// Accept resources with matching Susun ownership.
    AcceptOwned,
    /// Refuse if any target resource already exists.
    RefuseExisting,
}

/// Dependency wait handling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DependencyWaitPolicy {
    /// Emit wait actions for supported dependency conditions.
    EmitSupported,
    /// Refuse dependency conditions requiring waits.
    RefuseWaits,
    /// Degrade waits to start-order dependencies where safe.
    DegradeToStartOrder,
}

/// Options for `up` planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UpPlanOptions {
    /// Image acquisition policy.
    pub image_policy: ImageAcquisitionPolicy,
    /// Existing resource policy.
    pub existing_resource_policy: ExistingResourcePolicy,
    /// Dependency wait policy.
    pub dependency_wait_policy: DependencyWaitPolicy,
}

impl Default for UpPlanOptions {
    fn default() -> Self {
        Self {
            image_policy: ImageAcquisitionPolicy::PullIfMissing,
            existing_resource_policy: ExistingResourcePolicy::AcceptOwned,
            dependency_wait_policy: DependencyWaitPolicy::EmitSupported,
        }
    }
}

/// Options for `down` planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DownPlanOptions {
    /// Remove named volumes. Defaults to false.
    pub remove_volumes: bool,
    /// Remove owned orphan containers. Defaults to false.
    pub remove_orphans: bool,
    /// Stop timeout.
    pub timeout: Duration,
}

impl Default for DownPlanOptions {
    fn default() -> Self {
        Self {
            remove_volumes: false,
            remove_orphans: false,
            timeout: Duration::from_secs(10),
        }
    }
}
