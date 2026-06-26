//! Explicit convergence policy types.

/// Policy for image identity comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ImageChangePolicy {
    /// Compare image references only.
    #[default]
    ReferenceOnly,
    /// Use digest identity when the engine exposes one.
    DigestWhenAvailable,
    /// Refresh image identity according to pull policy.
    AlwaysRefreshAccordingToPullPolicy,
}

/// Policy for dependency impact after a service replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DependencyRecreatePolicy {
    /// Do not restart or recreate dependents by default.
    #[default]
    Never,
    /// Restart dependents that explicitly request dependency restart semantics.
    RestartExplicitDependents,
    /// Recreate dependents when the dependency contract requires it.
    RecreateWhenDependencyContractRequires,
}

/// Policy for anonymous volume handling during replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AnonymousVolumePolicy {
    /// Preserve anonymous volumes when target metadata matches.
    PreserveWhenTargetMatches,
    /// Recreate anonymous volumes.
    Recreate,
    /// Reject ambiguous anonymous volume ownership.
    #[default]
    RejectAmbiguous,
}

/// Policy for orphan reporting and removal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrphanPolicy {
    /// Report owned resources that no longer correspond to desired state.
    pub report: bool,
    /// Remove orphan containers.
    pub remove_containers: bool,
    /// Remove orphan networks.
    pub remove_networks: bool,
    /// Remove orphan volumes.
    pub remove_volumes: bool,
}

impl Default for OrphanPolicy {
    fn default() -> Self {
        Self {
            report: true,
            remove_containers: false,
            remove_networks: false,
            remove_volumes: false,
        }
    }
}

/// Container replacement strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ReplacementStrategy {
    /// Stop old, remove old, create new, start new.
    #[default]
    StopRemoveCreateStart,
    /// Create new resource first, then switch traffic/name where proven safe.
    CreateThenSwitch,
    /// Restart existing compatible container only.
    RestartOnly,
}

/// Recreate override policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RecreatePolicy {
    /// Recreate only when convergence detects relevant changes.
    #[default]
    Changed,
    /// Recreate every selected service instance.
    Force,
    /// Refuse recreates and report changes instead.
    Never,
}

/// Complete convergence policy bundle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConvergencePolicy {
    /// Image comparison policy.
    pub image_change: ImageChangePolicy,
    /// Dependency impact policy.
    pub dependency_recreate: DependencyRecreatePolicy,
    /// Anonymous volume preservation policy.
    pub anonymous_volumes: AnonymousVolumePolicy,
    /// Orphan handling policy.
    pub orphans: OrphanPolicy,
    /// Replacement strategy.
    pub replacement: ReplacementStrategy,
    /// Recreate override policy.
    pub recreate: RecreatePolicy,
    /// Renew anonymous volumes during recreation.
    pub renew_anonymous_volumes: bool,
}
