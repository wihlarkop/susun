//! Canonical project and service types.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::name::{ImageRef, ProjectName, ServiceName};

/// A parsed and normalised Compose service definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Service {
    /// Container image reference, if specified.
    pub image: Option<ImageRef>,
}

/// The top-level canonical Compose project.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Project {
    /// Resolved project name.
    pub name: ProjectName,
    /// Ordered map of service name → service definition.
    pub services: IndexMap<ServiceName, Service>,
}
