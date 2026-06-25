//! Plan schema versioning.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Version for serialized plan artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PlanSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl PlanSchemaVersion {
    /// Current Phase 2 schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

impl Default for PlanSchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}
