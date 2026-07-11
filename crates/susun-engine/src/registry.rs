//! Ephemeral registry authentication contracts.

use crate::ResourceNameError;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Stable reference to credentials owned by the embedding application.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct RegistryCredentialRef(String);

impl RegistryCredentialRef {
    /// Creates a non-secret credential reference.
    pub fn new(value: impl Into<String>) -> Result<Self, ResourceNameError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ResourceNameError::Empty);
        }
        Ok(Self(value))
    }
    /// Returns the reference value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Ephemeral credentials supplied immediately before an authenticated call.
pub struct RegistryAuthMaterial {
    username: Option<String>,
    password: Option<String>,
    identity_token: Option<String>,
    registry_token: Option<String>,
    server_address: Option<String>,
}

impl RegistryAuthMaterial {
    /// Creates username/password credentials.
    pub fn username_password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            identity_token: None,
            registry_token: None,
            server_address: None,
        }
    }
    /// Creates identity-token credentials.
    pub fn identity_token(token: impl Into<String>) -> Self {
        Self {
            username: None,
            password: None,
            identity_token: Some(token.into()),
            registry_token: None,
            server_address: None,
        }
    }
    /// Creates registry-token credentials.
    pub fn registry_token(token: impl Into<String>) -> Self {
        Self {
            username: None,
            password: None,
            identity_token: None,
            registry_token: Some(token.into()),
            server_address: None,
        }
    }
    /// Associates the credentials with a registry server.
    pub fn with_server_address(mut self, address: impl Into<String>) -> Self {
        self.server_address = Some(address.into());
        self
    }
    /// Returns the username for adapter use.
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }
    /// Returns the password for adapter use.
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
    /// Returns the identity token for adapter use.
    pub fn identity_token_value(&self) -> Option<&str> {
        self.identity_token.as_deref()
    }
    /// Returns the registry token for adapter use.
    pub fn registry_token_value(&self) -> Option<&str> {
        self.registry_token.as_deref()
    }
    /// Returns the server address for adapter use.
    pub fn server_address(&self) -> Option<&str> {
        self.server_address.as_deref()
    }
}

impl std::fmt::Debug for RegistryAuthMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryAuthMaterial")
            .field("username", &self.username.as_ref().map(|_| "[redacted]"))
            .field("password", &self.password.as_ref().map(|_| "[redacted]"))
            .field(
                "identity_token",
                &self.identity_token.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "registry_token",
                &self.registry_token.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "server_address",
                &self.server_address.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}
