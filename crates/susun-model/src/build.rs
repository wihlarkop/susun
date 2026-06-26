//! Canonical Compose build model.

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Canonical service build definition.
///
/// This type stores only Compose-level desired state. Filesystem resolution and
/// BuildKit-specific request construction live outside `susun-model`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BuildDefinition {
    /// Build context as written after interpolation.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub context: Option<String>,
    /// Dockerfile path.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub dockerfile: Option<String>,
    /// Target stage.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub target: Option<String>,
    /// Build arguments. A `None` value inherits from the build environment.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "IndexMap::is_empty")
    )]
    pub args: IndexMap<String, Option<String>>,
    /// Requested target platforms.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub platforms: Vec<String>,
    /// Secret identities referenced by the build.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub secrets: Vec<String>,
    /// SSH identities referenced by the build.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub ssh: Vec<String>,
    /// Cache sources.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub cache_from: Vec<String>,
    /// Cache destinations.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub cache_to: Vec<String>,
}
