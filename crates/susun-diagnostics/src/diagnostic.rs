//! Diagnostic and label types.

use susun_source::Span;

use crate::code::{DiagnosticCode, Severity};

/// An annotated source range within a diagnostic.
#[derive(Debug, Clone)]
pub struct Label {
    /// The source span this label highlights.
    pub span: Span,
    /// Human-readable message for this label.
    pub message: String,
    /// Whether this is a primary label (true) or a secondary/context label (false).
    pub primary: bool,
}

impl Label {
    /// Creates a primary label at the given span.
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Self { span, message: message.into(), primary: true }
    }

    /// Creates a secondary context label at the given span.
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Self { span, message: message.into(), primary: false }
    }
}

/// A single structured diagnostic with source location, severity, and labels.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Machine-readable code (e.g. `SUS-PARSE-001`).
    pub code: DiagnosticCode,
    /// Severity of this diagnostic.
    pub severity: Severity,
    /// Human-readable summary message.
    pub message: String,
    /// Source labels attached to this diagnostic.
    pub labels: Vec<Label>,
    /// Optional extended help text.
    pub help: Option<String>,
    /// Monotonically increasing insertion ordinal within the enclosing report.
    pub(crate) ordinal: u64,
}

impl Diagnostic {
    /// Creates a new diagnostic with no labels and no help text.
    pub fn new(
        code: impl Into<DiagnosticCode>,
        severity: Severity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            labels: Vec::new(),
            help: None,
            ordinal: 0,
        }
    }

    /// Attaches a label to this diagnostic and returns `self`.
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Attaches optional extended help text and returns `self`.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Returns the insertion ordinal within the enclosing report.
    ///
    /// Lower values were inserted first. Useful for verifying deterministic ordering.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
}

impl From<String> for DiagnosticCode {
    fn from(s: String) -> Self {
        DiagnosticCode::new(s)
    }
}

impl From<&str> for DiagnosticCode {
    fn from(s: &str) -> Self {
        DiagnosticCode::new(s)
    }
}
