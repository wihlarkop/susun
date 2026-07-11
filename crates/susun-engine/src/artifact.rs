//! Neutral image mutation and registry-push contracts.

use susun_model::ImageRef;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{ImageId, RegistryCredentialRef, ResourceNameError};

/// Schema version shared by artifact mutation results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ArtifactMutationSchemaVersion {
    /// Breaking schema generation.
    pub major: u16,
    /// Additive schema generation.
    pub minor: u16,
}

impl ArtifactMutationSchemaVersion {
    /// Current artifact mutation schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Image selector accepted by engine-wide mutations.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ImageSelector(String);

impl ImageSelector {
    /// Creates an image selector from an opaque engine ID or OCI-style reference.
    pub fn new(value: impl Into<String>) -> Result<Self, ResourceNameError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ResourceNameError::Empty);
        }
        Ok(Self(value))
    }

    /// Returns the selector string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ImageSelector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ImageSelector")
            .field(&self.0)
            .finish()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ImageSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Image removal request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageRemoveRequest {
    image: ImageSelector,
    force: bool,
    prune_children: bool,
}

impl ImageRemoveRequest {
    /// Creates a conservative image removal request.
    pub fn new(image: ImageSelector) -> Self {
        Self {
            image,
            force: false,
            prune_children: false,
        }
    }

    /// Enables forced removal.
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Enables removal of untagged parent images.
    pub fn with_prune_children(mut self, prune_children: bool) -> Self {
        self.prune_children = prune_children;
        self
    }

    /// Returns the target image.
    pub fn image(&self) -> &ImageSelector {
        &self.image
    }

    /// Returns whether forced removal is enabled.
    pub fn force(&self) -> bool {
        self.force
    }

    /// Returns whether untagged parent pruning is enabled.
    pub fn prune_children(&self) -> bool {
        self.prune_children
    }
}

/// Display-safe image removal result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageRemoveResult {
    /// Serialized contract version.
    pub schema_version: ArtifactMutationSchemaVersion,
    /// Removed image IDs.
    pub deleted: Vec<ImageId>,
    /// Removed tag references.
    pub untagged: Vec<ImageRef>,
}

/// Image tag request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageTagRequest {
    source: ImageSelector,
    target: ImageRef,
}

impl ImageTagRequest {
    /// Creates an image tag request.
    pub fn new(source: ImageSelector, target: ImageRef) -> Self {
        Self { source, target }
    }

    /// Returns the source image selector.
    pub fn source(&self) -> &ImageSelector {
        &self.source
    }

    /// Returns the target image reference.
    pub fn target(&self) -> &ImageRef {
        &self.target
    }
}

/// Display-safe image tag result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageTagResult {
    /// Serialized contract version.
    pub schema_version: ArtifactMutationSchemaVersion,
    /// Source selector used for the operation.
    pub source: ImageSelector,
    /// New image reference.
    pub target: ImageRef,
}

/// Anonymous registry push request. Credential references are added by the
/// separate registry boundary; this request never contains plaintext secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImagePushRequest {
    image: ImageRef,
    credential_ref: Option<RegistryCredentialRef>,
}

impl ImagePushRequest {
    /// Creates an anonymous push request.
    pub fn new(image: ImageRef) -> Self {
        Self {
            image,
            credential_ref: None,
        }
    }

    /// Selects credentials owned and resolved by the embedding application.
    pub fn with_credential_ref(mut self, credential_ref: RegistryCredentialRef) -> Self {
        self.credential_ref = Some(credential_ref);
        self
    }

    /// Returns the image reference to push.
    pub fn image(&self) -> &ImageRef {
        &self.image
    }

    /// Returns the non-secret credential reference, when authentication is requested.
    pub fn credential_ref(&self) -> Option<&RegistryCredentialRef> {
        self.credential_ref.as_ref()
    }
}

/// Display-safe registry push result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImagePushResult {
    /// Serialized contract version.
    pub schema_version: ArtifactMutationSchemaVersion,
    /// Pushed image reference.
    pub image: ImageRef,
    /// Immutable digest when reported by the registry.
    pub digest: Option<String>,
    /// Credential reference used for the push, never credential material.
    pub credential_ref: Option<RegistryCredentialRef>,
}
