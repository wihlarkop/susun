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
                    serde_json::json!({
                        "primary": label.primary,
                        "message": label.message,
                        "source": path,
                        "start": label.span.start.value(),
                        "end": label.span.end.value(),
                        "line": location.as_ref().map(|lc| lc.line),
                        "column": location.as_ref().map(|lc| lc.column),
                    })
                })
                .collect();
            serde_json::json!({
                "code": diag.code.as_str(),
                "severity": diag.severity.to_string(),
                "message": diag.message,
                "help": diag.help,
                "labels": labels,
            })
        })
        .collect();

    serde_json::to_string_pretty(&serde_json::json!({ "diagnostics": diagnostics }))
        .unwrap_or_else(|_| "{\"diagnostics\":[]}".to_owned())
}
