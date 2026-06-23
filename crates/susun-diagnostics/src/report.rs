//! Diagnostic report: collection and deterministic ordering.

use crate::{
    code::Severity,
    diagnostic::Diagnostic,
};

/// The sort key for deterministic diagnostic ordering.
///
/// Order: source declaration index → span start offset → severity (Error < Warning < Note)
/// → code string → insertion ordinal.
fn sort_key(d: &Diagnostic) -> impl Ord + '_ {
    let (source_idx, start_offset) = d
        .labels
        .first()
        .map(|l| (l.span.source_id.value(), l.span.start.value()))
        .unwrap_or((u32::MAX, u32::MAX));

    // Severity: lower numeric value = higher priority in sort (Error first)
    let severity_ord = match d.severity {
        Severity::Error => 0u8,
        Severity::Warning => 1,
        Severity::Note => 2,
    };

    (source_idx, start_offset, severity_ord, d.code.as_str(), d.ordinal)
}

/// An ordered collection of [`Diagnostic`]s.
///
/// Diagnostics are stored in insertion order internally and sorted on demand.
/// The sort is deterministic: source declaration index → span start → severity →
/// code → insertion ordinal.
#[derive(Debug, Default)]
pub struct DiagnosticReport {
    diagnostics: Vec<Diagnostic>,
    next_ordinal: u64,
}

impl DiagnosticReport {
    /// Creates an empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a diagnostic to this report.
    pub fn push(&mut self, mut diagnostic: Diagnostic) {
        diagnostic.ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        self.diagnostics.push(diagnostic);
    }

    /// Returns `true` if any diagnostic has [`Severity::Error`].
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// Returns the total number of diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns `true` if no diagnostics have been recorded.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns diagnostics in deterministic sort order.
    pub fn sorted(&self) -> Vec<&Diagnostic> {
        let mut refs: Vec<&Diagnostic> = self.diagnostics.iter().collect();
        refs.sort_by_key(|d| sort_key(d));
        refs
    }

    /// Merges all diagnostics from `other` into this report, preserving relative ordinals.
    pub fn merge(&mut self, other: DiagnosticReport) {
        for d in other.diagnostics {
            self.push(d);
        }
    }

    /// Returns an iterator over all diagnostics in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }
}
