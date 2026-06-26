//! Fingerprint schema constants and value types.

use std::fmt;

use crate::ConvergenceError;

/// Current supported configuration fingerprint schema version.
pub const CURRENT_FINGERPRINT_VERSION: u16 = 1;

/// Stable digest algorithm for fingerprint version 1.
pub const FINGERPRINT_ALGORITHM: &str = "sha256";

/// Runtime label prefix for Susun configuration fingerprints.
pub const FINGERPRINT_LABEL_PREFIX: &str = "susun-fp";

/// Supported fingerprint schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerprintVersion(u16);

impl FingerprintVersion {
    /// Current fingerprint version.
    pub const CURRENT: Self = Self(CURRENT_FINGERPRINT_VERSION);

    /// Creates a version value.
    pub fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the numeric version.
    pub fn as_u16(self) -> u16 {
        self.0
    }

    /// Returns whether this version is supported by the current crate.
    pub fn is_supported(self) -> bool {
        self == Self::CURRENT
    }
}

impl fmt::Display for FingerprintVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Hex-encoded digest for a canonical fingerprint input.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerprintDigest(String);

impl FingerprintDigest {
    /// Creates a digest from lowercase hexadecimal SHA-256 output.
    pub fn new(value: impl Into<String>) -> Result<Self, ConvergenceError> {
        let value = value.into();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ConvergenceError::FingerprintInvariant {
                detail: "fingerprint digest must be 64 hexadecimal characters".to_string(),
            });
        }

        Ok(Self(value.to_ascii_lowercase()))
    }

    /// Returns the digest string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for FingerprintDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FingerprintDigest")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for FingerprintDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parsed versioned configuration fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedFingerprint {
    /// Fingerprint schema version.
    pub version: FingerprintVersion,
    /// Digest algorithm name.
    pub algorithm: &'static str,
    /// Canonical digest.
    pub digest: FingerprintDigest,
}

impl VersionedFingerprint {
    /// Creates a current-version fingerprint.
    pub fn current(digest: FingerprintDigest) -> Self {
        Self {
            version: FingerprintVersion::CURRENT,
            algorithm: FINGERPRINT_ALGORITHM,
            digest,
        }
    }

    /// Formats this fingerprint for the engine label value.
    pub fn label_value(&self) -> String {
        format!(
            "{}-{}:{}:{}",
            FINGERPRINT_LABEL_PREFIX, self.version, self.algorithm, self.digest
        )
    }
}
