//! Neutral engine resource identifiers.

use std::fmt;

use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Error returned when a runtime resource name is invalid.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResourceNameError {
    /// Runtime resource names must not be empty.
    #[error("resource name must not be empty")]
    Empty,
}

/// Runtime-visible resource name.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ResourceName(String);

impl ResourceName {
    /// Creates a runtime resource name.
    pub fn new(value: impl Into<String>) -> Result<Self, ResourceNameError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ResourceNameError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the resource name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResourceName").field(&self.as_str()).finish()
    }
}

impl fmt::Display for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ResourceName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
