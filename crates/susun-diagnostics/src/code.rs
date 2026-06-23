//! Diagnostic codes and severity levels.

use std::fmt;

/// A structured diagnostic code (e.g. `SUS-PARSE-001`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    /// Creates a new diagnostic code from any string-like value.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// Returns the code as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for DiagnosticCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Informational note; does not affect outcome.
    Note,
    /// Warning; analysis proceeds but the result may be surprising.
    Warning,
    /// Error; the project cannot be used reliably.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Note => f.write_str("note"),
            Self::Warning => f.write_str("warning"),
            Self::Error => f.write_str("error"),
        }
    }
}
