//! Minimal diagnostic renderer for Phase 1.
//!
//! Task 31 replaces this with a full human-readable renderer that includes
//! source excerpts and caret highlighting.

use serde::{Deserialize, Serialize, de::Error as _};
use susun_diagnostics::{DiagnosticReport, Severity};
use susun_source::SourceMap;

/// Serializable diagnostics document for SDK consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticReportSummary {
    /// Serialized diagnostics summary schema version.
    pub schema_version: DiagnosticReportSummarySchemaVersion,
    /// Whether any diagnostic has error severity.
    pub has_errors: bool,
    /// Number of diagnostics in deterministic order.
    pub diagnostic_count: usize,
    /// Diagnostics in deterministic order.
    pub diagnostics: Vec<DiagnosticSummary>,
}

/// Serialized diagnostics summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticReportSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl DiagnosticReportSummarySchemaVersion {
    /// Current diagnostics summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Serializable diagnostic summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticSummary {
    /// Machine-readable diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional extended help text.
    pub help: Option<String>,
    /// Source labels in diagnostic order.
    pub labels: Vec<DiagnosticLabelSummary>,
}

/// Serializable diagnostic label summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticLabelSummary {
    /// Whether this is the primary label.
    pub primary: bool,
    /// Human-readable label message.
    pub message: String,
    /// Display path when the source is file-backed.
    pub source: Option<String>,
    /// Start byte offset.
    pub start: u32,
    /// End byte offset.
    pub end: u32,
    /// One-based line when resolvable.
    pub line: Option<u32>,
    /// One-based column when resolvable.
    pub column: Option<u32>,
}

/// Renders all diagnostics in `report` to a plain-text string.
///
/// Each diagnostic is formatted as `severity[CODE]: message` followed by
/// `  --> path:line:col` for each attached label. Diagnostics are emitted
/// in deterministic sort order (source position, then severity, then code).
pub fn render_diagnostics(report: &DiagnosticReport, source_map: &SourceMap) -> String {
    let mut out = String::new();
    for diag in report.sorted() {
        let prefix = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        out.push_str(&format!(
            "{}[{}]: {}\n",
            prefix,
            diag.code.as_str(),
            diag.message
        ));
        for label in &diag.labels {
            if let Ok(lc) = source_map.resolve(label.span.source_id, label.span.start) {
                let path_str = source_map
                    .get(label.span.source_id)
                    .and_then(|s| s.path.as_ref())
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<unknown>".to_owned());
                out.push_str(&format!("  --> {}:{}:{}\n", path_str, lc.line, lc.column));
            }
            if !label.message.is_empty() {
                out.push_str(&format!("     = {}\n", label.message));
            }
        }
    }
    out
}

/// Renders diagnostics as stable JSON.
pub fn render_diagnostics_json(report: &DiagnosticReport, source_map: &SourceMap) -> String {
    render_diagnostic_report_summary_json(&diagnostic_report_summary(report, source_map))
        .unwrap_or_else(|_| "{\"schema_version\":{\"major\":1,\"minor\":0},\"has_errors\":false,\"diagnostic_count\":0,\"diagnostics\":[]}".to_owned())
}

/// Builds a serializable diagnostics summary from a report and source map.
pub fn diagnostic_report_summary(
    report: &DiagnosticReport,
    source_map: &SourceMap,
) -> DiagnosticReportSummary {
    let diagnostics: Vec<_> = report
        .sorted()
        .into_iter()
        .map(|diag| {
            let labels: Vec<_> = diag
                .labels
                .iter()
                .map(|label| {
                    let location = source_map
                        .resolve(label.span.source_id, label.span.start)
                        .ok();
                    let path = source_map
                        .get(label.span.source_id)
                        .and_then(|source| source.path.as_ref())
                        .map(|path| path.display().to_string());
                    DiagnosticLabelSummary {
                        primary: label.primary,
                        message: label.message.clone(),
                        source: path,
                        start: label.span.start.value(),
                        end: label.span.end.value(),
                        line: location.as_ref().map(|lc| lc.line),
                        column: location.as_ref().map(|lc| lc.column),
                    }
                })
                .collect();
            DiagnosticSummary {
                code: diag.code.as_str().to_owned(),
                severity: diag.severity.to_string(),
                message: diag.message.clone(),
                help: diag.help.clone(),
                labels,
            }
        })
        .collect();

    DiagnosticReportSummary {
        schema_version: DiagnosticReportSummarySchemaVersion::CURRENT,
        has_errors: report.has_errors(),
        diagnostic_count: diagnostics.len(),
        diagnostics,
    }
}

/// Renders a diagnostics summary as pretty JSON using the public SDK schema.
pub fn render_diagnostic_report_summary_json(
    summary: &DiagnosticReportSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a diagnostics summary from JSON using the public SDK schema.
pub fn parse_diagnostic_report_summary_json(
    input: &str,
) -> Result<DiagnosticReportSummary, serde_json::Error> {
    let summary: DiagnosticReportSummary = serde_json::from_str(input)?;
    validate_diagnostic_report_summary(&summary)?;
    Ok(summary)
}

fn validate_diagnostic_report_summary(
    summary: &DiagnosticReportSummary,
) -> Result<(), serde_json::Error> {
    if summary.schema_version != DiagnosticReportSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported diagnostic report summary schema version {}.{}",
            summary.schema_version.major, summary.schema_version.minor
        )));
    }
    if summary.diagnostic_count != summary.diagnostics.len() {
        return Err(serde_json::Error::custom(
            "diagnostic report summary count does not match diagnostics",
        ));
    }
    if summary.has_errors
        != summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity.eq_ignore_ascii_case("error"))
    {
        return Err(serde_json::Error::custom(
            "diagnostic report summary error flag does not match diagnostics",
        ));
    }
    for diagnostic in &summary.diagnostics {
        if !matches!(diagnostic.severity.as_str(), "error" | "warning" | "note") {
            return Err(serde_json::Error::custom(
                "diagnostic report summary contains unknown severity",
            ));
        }
        for label in &diagnostic.labels {
            if label.start > label.end {
                return Err(serde_json::Error::custom(
                    "diagnostic report summary label start exceeds end",
                ));
            }
            if label.line.is_some() != label.column.is_some() {
                return Err(serde_json::Error::custom(
                    "diagnostic report summary label line and column must be present together",
                ));
            }
        }
    }
    Ok(())
}
