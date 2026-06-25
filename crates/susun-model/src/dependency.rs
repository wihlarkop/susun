//! Canonical service dependency model.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::ServiceName;

/// Compose dependency condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DependencyCondition {
    /// Dependency must be started.
    #[default]
    ServiceStarted,
    /// Dependency must report healthy.
    ServiceHealthy,
    /// Dependency must complete successfully.
    ServiceCompletedSuccessfully,
}

/// A normalized dependency on another service.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ServiceDependency {
    /// Required dependency condition.
    pub condition: DependencyCondition,
    /// Whether this dependency restarts with the source service.
    pub restart: bool,
    /// Whether the dependency is required.
    pub required: bool,
}

impl Default for ServiceDependency {
    fn default() -> Self {
        Self {
            condition: DependencyCondition::ServiceStarted,
            restart: false,
            required: true,
        }
    }
}

/// Service dependency map keyed by dependency service name.
pub type Dependencies = IndexMap<ServiceName, ServiceDependency>;
