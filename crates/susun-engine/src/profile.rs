//! Neutral engine connection profile types.

use std::sync::Arc;

use crate::EngineEndpoint;

/// Stable identifier for a user-visible engine connection profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EngineConnectionProfileId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Human-readable display name for an engine connection profile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EngineConnectionDisplayName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
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
    default: bool,
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
            default: false,
        }
    }

    /// Creates the conventional local Docker-compatible default profile.
    pub fn local_default() -> Self {
        Self {
            id: EngineConnectionProfileId(Arc::from("local")),
            display_name: EngineConnectionDisplayName(Arc::from("Local Docker-compatible runtime")),
            endpoint: EngineEndpoint::Local,
            default: true,
        }
    }

    /// Marks the profile as the default runtime profile.
    pub fn with_default(mut self, default: bool) -> Self {
        self.default = default;
        self
    }

    /// Returns whether this profile is marked as the default.
    pub fn is_default(&self) -> bool {
        self.default
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
            .field("default", &self.default)
            .finish()
    }
}

/// Ordered collection of engine connection profiles with validated defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EngineConnectionProfileSet {
    profiles: Vec<EngineConnectionProfile>,
}

impl EngineConnectionProfileSet {
    /// Creates a validated profile set.
    pub fn new(
        profiles: Vec<EngineConnectionProfile>,
    ) -> Result<Self, EngineConnectionProfileError> {
        for (index, profile) in profiles.iter().enumerate() {
            if profiles[..index]
                .iter()
                .any(|candidate| candidate.id == profile.id)
            {
                return Err(EngineConnectionProfileError::DuplicateId(
                    profile.id.clone(),
                ));
            }
        }
        if profiles.iter().filter(|profile| profile.default).count() > 1 {
            return Err(EngineConnectionProfileError::MultipleDefaults);
        }
        Ok(Self { profiles })
    }

    /// Returns the profiles in insertion order.
    pub fn profiles(&self) -> &[EngineConnectionProfile] {
        &self.profiles
    }

    /// Returns the selected default profile.
    ///
    /// If no profile is explicitly marked default, the first profile is used.
    pub fn default_profile(&self) -> Option<&EngineConnectionProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.default)
            .or_else(|| self.profiles.first())
    }

    /// Finds a profile by id.
    pub fn get(&self, id: &EngineConnectionProfileId) -> Option<&EngineConnectionProfile> {
        self.profiles.iter().find(|profile| &profile.id == id)
    }

    /// Returns whether the set has no profiles.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EngineConnectionProfileSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct ProfileSet {
            profiles: Vec<EngineConnectionProfile>,
        }

        let value = <ProfileSet as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(value.profiles).map_err(serde::de::Error::custom)
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
    /// Profile ids must be unique in a profile set.
    #[error("duplicate engine connection profile id: {0}")]
    DuplicateId(EngineConnectionProfileId),
    /// At most one profile may be marked as default.
    #[error("only one engine connection profile may be marked default")]
    MultipleDefaults,
}
