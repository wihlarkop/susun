//! Action reasons and safety labels.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Machine-readable reason for a planned action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ActionReason {
    /// Required resource is missing.
    ResourceMissing,
    /// Image is unavailable locally.
    ImageUnavailableLocally,
    /// Service was explicitly requested.
    ServiceRequested,
    /// Action is needed by a dependency.
    DependencyRequired,
    /// Teardown was requested.
    TeardownRequested,
    /// Orphan removal was requested.
    OrphanRemovalRequested,
    /// Existing compatible resource is accepted.
    ExistingResourceAccepted,
    /// Engine capability limits affect the action.
    CapabilityConstraint,
}

/// Human-readable and machine-readable action explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ActionExplanation {
    /// Machine-readable reason code.
    pub code: ActionReason,
    /// Human-readable reason text.
    pub message: String,
}

impl ActionExplanation {
    /// Creates an action explanation.
    pub fn new(code: ActionReason, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Safety classification for a planned action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ActionSafety {
    /// Non-destructive action.
    Safe,
    /// Requires user attention but is not destructive by itself.
    Caution,
    /// Destructive action.
    Destructive,
}
