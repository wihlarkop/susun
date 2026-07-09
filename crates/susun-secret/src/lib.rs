//! Secret-handling utilities for public Susun artifacts and logs.

use std::{fmt, str};

/// Stable redaction marker used in user-facing artifacts.
pub const REDACTED: &str = "<redacted>";

/// Owned secret bytes that never reveal contents through formatting or serde.
///
/// When the `serde` feature is enabled, this type serializes only as the stable
/// redaction marker. It intentionally does not deserialize because a redacted
/// artifact cannot reconstruct the original secret and accepting arbitrary text
/// here would make the redaction boundary ambiguous.
#[derive(Default, Eq, PartialEq)]
pub struct RedactedSecret {
    bytes: Vec<u8>,
}

impl RedactedSecret {
    /// Creates a secret from owned bytes.
    #[must_use]
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Creates a secret from UTF-8 text.
    #[must_use]
    pub fn from_string(value: impl Into<String>) -> Self {
        Self::from_bytes(value.into().into_bytes())
    }

    /// Returns whether the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the secret length in bytes without exposing the value.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Exposes the underlying bytes to provider code that must materialize them.
    ///
    /// Callers must not format, serialize, or log the returned bytes.
    #[must_use]
    pub fn expose_secret(&self) -> &[u8] {
        &self.bytes
    }
}

impl Clone for RedactedSecret {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
        }
    }
}

impl Drop for RedactedSecret {
    fn drop(&mut self) {
        for byte in &mut self.bytes {
            *byte = 0;
        }
    }
}

impl fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl fmt::Display for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl From<String> for RedactedSecret {
    fn from(value: String) -> Self {
        Self::from_string(value)
    }
}

impl From<&str> for RedactedSecret {
    fn from(value: &str) -> Self {
        Self::from_bytes(value.as_bytes().to_vec())
    }
}

impl From<Vec<u8>> for RedactedSecret {
    fn from(value: Vec<u8>) -> Self {
        Self::from_bytes(value)
    }
}

impl From<&[u8]> for RedactedSecret {
    fn from(value: &[u8]) -> Self {
        Self::from_bytes(value.to_vec())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RedactedSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(REDACTED)
    }
}

/// Redacts text that appears to contain credential material.
#[must_use]
pub fn redact_sensitive_text(input: &str) -> String {
    if contains_sensitive_marker(input) {
        REDACTED.to_owned()
    } else {
        input.to_owned()
    }
}

/// Returns whether text appears to contain credential-bearing material.
#[must_use]
pub fn contains_sensitive_marker(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    const SUBSTRING_MARKERS: &[&str] = &[
        "authorization",
        "credential",
        "passwd",
        "password",
        "private_key",
        "secret",
        "token",
        "connection_string",
        "conn_str",
        "database_url",
        "db_url",
    ];
    const TOKEN_MARKERS: &[&str] = &[
        "auth", "bearer", "cert", "cookie", "dsn", "jwt", "key", "session",
    ];

    SUBSTRING_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
        || lower
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|token| TOKEN_MARKERS.contains(&token))
}

#[cfg(test)]
mod tests {
    use super::{REDACTED, RedactedSecret, contains_sensitive_marker};

    #[test]
    fn formatting_never_exposes_secret_bytes() {
        let secret = RedactedSecret::from_string("super-secret-value");

        assert_eq!(secret.to_string(), REDACTED);
        assert_eq!(format!("{secret:?}"), REDACTED);
        assert!(!secret.is_empty());
        assert_eq!(secret.len(), "super-secret-value".len());
        assert_eq!(secret.expose_secret(), b"super-secret-value");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_serializes_only_the_redaction_marker() -> Result<(), serde_json::Error> {
        let secret = RedactedSecret::from_string("super-secret-value");

        assert_eq!(serde_json::to_string(&secret)?, format!("\"{REDACTED}\""));
        Ok(())
    }

    #[test]
    fn matches_previously_uncaught_markers() {
        for key in [
            "API_KEY",
            "DATABASE_URL",
            "DSN",
            "CONNECTION_STRING",
            "CONN_STR",
            "DB_URL",
            "TLS_CERT",
            "SESSION_ID",
            "COOKIE_SECRET",
            "JWT_SECRET",
            "BEARER_TOKEN",
            "OAUTH_CLIENT_SECRET",
        ] {
            assert!(
                contains_sensitive_marker(key),
                "expected `{key}` to be flagged as sensitive"
            );
        }
    }

    #[test]
    fn leaves_ordinary_keys_alone() {
        for key in [
            "NODE_ENV",
            "PORT",
            "LOG_LEVEL",
            "TZ",
            "DEBUG",
            "MONKEY_PATCH",
            "AUTHOR_NAME",
            "CONCERT_MODE",
            "CERTAINLY_ENABLED",
            "KEYSTONE_PROJECT",
        ] {
            assert!(
                !contains_sensitive_marker(key),
                "expected `{key}` to NOT be flagged as sensitive"
            );
        }
    }
}
