//! Canonical healthcheck model.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::Command;

/// Supported Compose healthcheck subset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Healthcheck {
    /// Healthcheck command.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub test: Option<Command>,
    /// Interval duration as written in Compose.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub interval: Option<String>,
    /// Timeout duration as written in Compose.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub timeout: Option<String>,
    /// Start period duration as written in Compose.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub start_period: Option<String>,
    /// Retry count.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub retries: Option<u32>,
    /// Whether the image healthcheck is disabled.
    pub disable: bool,
}
