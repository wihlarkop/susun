//! Canonical project type.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    name::{ProjectName, ServiceName},
    service::Service,
};

/// The top-level canonical Compose project.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Project {
    /// Resolved project name.
    pub name: ProjectName,
    /// Ordered map of service name → service definition.
    pub services: IndexMap<ServiceName, Service>,
}
