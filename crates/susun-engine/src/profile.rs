//! Neutral engine connection profile types.

use std::sync::Arc;

use crate::EngineEndpoint;

/// Stable identifier for a user-visible engine connection profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EngineConnectionProfileId(Arc<str>);

impl EngineConnectionProfileId {
    /// Creates a profile id. IDs must be non-empty and contain no
    /// whitespace so they are safe for database keys and CLI arguments.
    pub fn new(value: impl Into<Arc<str>>) -> Result<Self, EngineConnectionProfileError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EngineConnectionProfileError::EmptyId);
        }
        if value.chars().any(char::is_whitespace) {
            return Err(EngineConnectionProfileError::InvalidId);
        }
        Ok(Self(value))
    }

    /// Returns the profile id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EngineConnectionProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Human-readable display name for an engine connection profile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EngineConnectionDisplayName(Arc<str>);

impl EngineConnectionDisplayName {
    /// Creates a display name. Leading/trailing whitespace is trimmed.
    pub fn new(value: impl Into<Arc<str>>) -> Result<Self, EngineConnectionProfileError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(EngineConnectionProfileError::EmptyDisplayName);
        }
        Ok(Self(Arc::from(trimmed)))
    }

    /// Returns the display name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EngineConnectionDisplayName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Named engine connection profile used by UIs and daemons.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineConnectionProfile {
    /// Stable profile id.
    pub id: EngineConnectionProfileId,
    /// User-visible display name.
    pub display_name: EngineConnectionDisplayName,
    endpoint: EngineEndpoint,
}

impl EngineConnectionProfile {
    /// Creates a connection profile from already-validated values.
    pub fn new(
        id: EngineConnectionProfileId,
        display_name: EngineConnectionDisplayName,
        endpoint: EngineEndpoint,
    ) -> Self {
        Self {
            id,
            display_name,
            endpoint,
        }
    }

    /// Returns the configured endpoint.
    pub fn endpoint(&self) -> &EngineEndpoint {
        &self.endpoint
    }

    /// Returns the display-safe endpoint string.
    pub fn redacted_endpoint(&self) -> String {
        self.endpoint.redacted()
    }
}

impl std::fmt::Debug for EngineConnectionProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineConnectionProfile")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("endpoint", &self.endpoint.redacted())
            .finish()
    }
}

/// Validation failure for connection profile identity/display values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineConnectionProfileError {
    /// Profile id was empty.
    #[error("engine connection profile id must not be empty")]
    EmptyId,
    /// Profile id contained whitespace.
    #[error("engine connection profile id must not contain whitespace")]
    InvalidId,
    /// Display name was empty.
    #[error("engine connection profile display name must not be empty")]
    EmptyDisplayName,
}
