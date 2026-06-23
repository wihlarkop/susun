//! Minimal diagnostic renderer for Phase 1.
//!
//! Task 31 replaces this with a full human-readable renderer that includes
//! source excerpts and caret highlighting.

use susun_diagnostics::{DiagnosticReport, Severity};
use susun_source::SourceMap;

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
        out.push_str(&format!("{}[{}]: {}\n", prefix, diag.code.as_str(), diag.message));
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
