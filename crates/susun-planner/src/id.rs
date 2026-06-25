//! Stable plan and action identifiers.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Stable execution plan ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PlanId(String);

impl PlanId {
    /// Creates a plan ID from canonical parts.
    pub fn from_parts(parts: &[&str]) -> Self {
        Self(format!("plan-{:016x}", StableIdBuilder::hash(parts)))
    }

    /// Returns the ID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable action ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ActionId(String);

impl ActionId {
    /// Creates an action ID from canonical parts.
    pub fn from_parts(parts: &[&str]) -> Self {
        Self(format!("act-{:016x}", StableIdBuilder::hash(parts)))
    }

    /// Returns the ID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable byte-oriented hash builder for IDs.
#[derive(Debug, Clone)]
pub struct StableIdBuilder {
    state: u64,
}

impl StableIdBuilder {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    /// Creates a new ID builder.
    pub fn new() -> Self {
        Self {
            state: Self::OFFSET,
        }
    }

    /// Adds one canonical part.
    pub fn part(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.bytes(&[0]);
    }

    /// Finishes the hash.
    pub fn finish(self) -> u64 {
        self.state
    }

    /// Hashes canonical parts in order.
    pub fn hash(parts: &[&str]) -> u64 {
        let mut builder = Self::new();
        for part in parts {
            builder.part(part);
        }
        builder.finish()
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }
}

impl Default for StableIdBuilder {
    fn default() -> Self {
        Self::new()
    }
}
