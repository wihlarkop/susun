//! Machine-readable Susun capability matrix.

use indexmap::IndexMap;

/// Schema version for published capability matrices.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CapabilityMatrixSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl CapabilityMatrixSchemaVersion {
    /// Current capability matrix schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

impl Default for CapabilityMatrixSchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

/// Support level for a public Susun feature.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FeatureSupport {
    /// Fully supported for the documented scope.
    Supported,
    /// Supported for a documented subset.
    SupportedSubset,
    /// Available but still intentionally unstable.
    Experimental,
    /// Known to be unavailable.
    Unsupported,
}

/// One feature entry in the machine-readable capability matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FeatureCapability {
    /// Support level for the feature.
    pub support: FeatureSupport,
    /// Short scope statement for humans and release tooling.
    pub scope: String,
}

impl FeatureCapability {
    /// Creates a feature capability entry.
    #[must_use]
    pub fn new(support: FeatureSupport, scope: impl Into<String>) -> Self {
        Self {
            support,
            scope: scope.into(),
        }
    }
}

/// Published capability matrix for a Susun release or development snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CapabilityMatrix {
    /// Matrix schema version.
    pub schema_version: CapabilityMatrixSchemaVersion,
    /// Susun version this matrix describes.
    pub susun_version: String,
    /// Compose reference version used for compatibility claims.
    pub compose_reference: String,
    /// Deterministically ordered feature map.
    pub features: IndexMap<String, FeatureCapability>,
}

impl CapabilityMatrix {
    /// Creates an empty capability matrix.
    #[must_use]
    pub fn new(susun_version: impl Into<String>, compose_reference: impl Into<String>) -> Self {
        Self {
            schema_version: CapabilityMatrixSchemaVersion::CURRENT,
            susun_version: susun_version.into(),
            compose_reference: compose_reference.into(),
            features: IndexMap::new(),
        }
    }

    /// Adds a feature entry while preserving insertion order.
    #[must_use]
    pub fn with_feature(
        mut self,
        key: impl Into<String>,
        support: FeatureSupport,
        scope: impl Into<String>,
    ) -> Self {
        self.features
            .insert(key.into(), FeatureCapability::new(support, scope));
        self
    }
}

/// Builds the current Phase 5 capability matrix.
#[must_use]
pub fn matrix_for_current_phase(
    susun_version: impl Into<String>,
    compose_reference: impl Into<String>,
) -> CapabilityMatrix {
    use FeatureSupport::{Supported, SupportedSubset, Unsupported};

    CapabilityMatrix::new(susun_version, compose_reference)
        .with_feature(
            "services.build.context",
            Supported,
            "Project-relative build contexts with deterministic manifests.",
        )
        .with_feature(
            "services.build.dockerfile",
            Supported,
            "Dockerfile path resolution and Level A validation.",
        )
        .with_feature(
            "services.build.target",
            Supported,
            "Target syntax validation and buildx argument emission.",
        )
        .with_feature(
            "services.build.args",
            Supported,
            "Resolved Compose build arguments are passed to the build engine.",
        )
        .with_feature(
            "services.build.secrets",
            SupportedSubset,
            "Secret identities are modeled and redacted; provider materialization is limited.",
        )
        .with_feature(
            "services.build.ssh",
            SupportedSubset,
            "SSH forwarding identities are modeled for buildx transport.",
        )
        .with_feature(
            "services.build.cache",
            SupportedSubset,
            "Cache import/export specs are preserved for buildx transport.",
        )
        .with_feature(
            "services.build.platforms",
            SupportedSubset,
            "Platform requests are modeled for build execution where the backend supports them.",
        )
        .with_feature(
            "dockerignore",
            Supported,
            "Ordered pattern matching and deterministic context filtering.",
        )
        .with_feature(
            "include",
            SupportedSubset,
            "Advanced Compose loading subset with deterministic provenance and limits.",
        )
        .with_feature(
            "extends",
            SupportedSubset,
            "Service inheritance subset with deterministic merge behavior.",
        )
        .with_feature(
            "merge_tags.reset_override",
            SupportedSubset,
            "Explicit handling for supported advanced merge tags.",
        )
        .with_feature(
            "configs",
            SupportedSubset,
            "Canonical modeling and Docker runtime materialization subset.",
        )
        .with_feature(
            "secrets",
            SupportedSubset,
            "Canonical modeling and redacted Docker runtime materialization subset.",
        )
        .with_feature(
            "runtime.run",
            SupportedSubset,
            "One-off service containers with cleanup policy.",
        )
        .with_feature(
            "runtime.exec",
            SupportedSubset,
            "Command execution in selected running service containers.",
        )
        .with_feature(
            "runtime.events",
            SupportedSubset,
            "Neutral project event stream with service filtering.",
        )
        .with_feature(
            "runtime.wait",
            SupportedSubset,
            "Waits for selected project service containers to exit.",
        )
        .with_feature(
            "runtime.cp",
            SupportedSubset,
            "Explicit host/container copy semantics for selected service containers.",
        )
        .with_feature(
            "runtime.port",
            Supported,
            "Published host port lookup for selected services.",
        )
        .with_feature(
            "watch.develop",
            SupportedSubset,
            "Rebuild, restart, sync, and sync-restart actions with safe file event normalization.",
        )
        .with_feature(
            "deploy",
            Unsupported,
            "Swarm deploy semantics remain out of scope.",
        )
        .with_feature(
            "kubernetes",
            Unsupported,
            "Kubernetes conversion remains out of scope.",
        )
}
