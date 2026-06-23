//! Tests for DiagnosticReport ordering, merging, and error detection.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use susun_diagnostics::{Diagnostic, DiagnosticReport, Label, Severity};
use susun_source::{LoadedSource, SourceMap, SourceName, Span, TextOffset};

fn make_map_with_sources() -> (SourceMap, susun_source::SourceId, susun_source::SourceId) {
    let mut map = SourceMap::new();
    let id0 = map.register(LoadedSource {
        name: SourceName::new("first.yaml"),
        path: None,
        contents: Arc::from("name: app\n"),
    });
    let id1 = map.register(LoadedSource {
        name: SourceName::new("second.yaml"),
        path: None,
        contents: Arc::from("services:\n  web:\n"),
    });
    (map, id0, id1)
}

fn span(source_id: susun_source::SourceId, start: u32, end: u32) -> Span {
    Span::new(source_id, TextOffset::new(start), TextOffset::new(end)).unwrap()
}

fn error(code: &str, source_id: susun_source::SourceId, start: u32) -> Diagnostic {
    Diagnostic::new(code, Severity::Error, "an error")
        .with_label(Label::primary(span(source_id, start, start + 1), "here"))
}

fn warning(code: &str, source_id: susun_source::SourceId, start: u32) -> Diagnostic {
    Diagnostic::new(code, Severity::Warning, "a warning")
        .with_label(Label::primary(span(source_id, start, start + 1), "here"))
}

// ── has_errors ────────────────────────────────────────────────────────────────

#[test]
fn has_errors_false_when_empty() {
    let report = DiagnosticReport::new();
    assert!(!report.has_errors());
}

#[test]
fn has_errors_false_for_warnings_only() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    report.push(warning("SUS-WARN-001", id0, 0));
    assert!(!report.has_errors());
}

#[test]
fn has_errors_true_when_error_present() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    report.push(error("SUS-PARSE-001", id0, 0));
    assert!(report.has_errors());
}

// ── count ─────────────────────────────────────────────────────────────────────

#[test]
fn len_reflects_push_count() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    assert_eq!(report.len(), 0);
    report.push(error("SUS-PARSE-001", id0, 0));
    assert_eq!(report.len(), 1);
    report.push(warning("SUS-WARN-001", id0, 2));
    assert_eq!(report.len(), 2);
}

// ── merge ─────────────────────────────────────────────────────────────────────

#[test]
fn merge_combines_diagnostics() {
    let (_, id0, id1) = make_map_with_sources();
    let mut a = DiagnosticReport::new();
    a.push(error("SUS-PARSE-001", id0, 0));

    let mut b = DiagnosticReport::new();
    b.push(warning("SUS-WARN-001", id1, 0));

    a.merge(b);
    assert_eq!(a.len(), 2);
    assert!(a.has_errors());
}

// ── ordering ──────────────────────────────────────────────────────────────────

#[test]
fn sorted_orders_by_source_then_offset() {
    let (_, id0, id1) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    // push in reverse order: second source first, higher offset first
    report.push(error("SUS-A", id1, 5));
    report.push(error("SUS-B", id0, 3));
    report.push(error("SUS-C", id0, 1));

    let sorted = report.sorted();
    assert_eq!(sorted[0].code.as_str(), "SUS-C"); // id0, offset 1
    assert_eq!(sorted[1].code.as_str(), "SUS-B"); // id0, offset 3
    assert_eq!(sorted[2].code.as_str(), "SUS-A"); // id1, offset 5
}

#[test]
fn sorted_errors_before_warnings_at_same_location() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    report.push(warning("SUS-WARN", id0, 0));
    report.push(error("SUS-ERR", id0, 0));

    let sorted = report.sorted();
    assert_eq!(sorted[0].severity, Severity::Error);
    assert_eq!(sorted[1].severity, Severity::Warning);
}

#[test]
fn sorted_uses_code_as_tiebreaker() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    report.push(error("SUS-Z", id0, 0));
    report.push(error("SUS-A", id0, 0));

    let sorted = report.sorted();
    assert_eq!(sorted[0].code.as_str(), "SUS-A");
    assert_eq!(sorted[1].code.as_str(), "SUS-Z");
}

#[test]
fn sorted_uses_ordinal_as_final_tiebreaker() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    // Two diagnostics with identical code, location, and severity — ordinal decides
    report.push(error("SUS-DUP", id0, 0));
    report.push(error("SUS-DUP", id0, 0));

    let sorted = report.sorted();
    assert_eq!(sorted.len(), 2);
    assert!(sorted[0].ordinal() < sorted[1].ordinal());
}

#[test]
fn diagnostics_without_labels_sort_last() {
    let (_, id0, _) = make_map_with_sources();
    let mut report = DiagnosticReport::new();
    let no_label = Diagnostic::new("SUS-NO-SPAN", Severity::Error, "no location");
    let with_label = error("SUS-SPAN", id0, 0);
    report.push(no_label);
    report.push(with_label);

    let sorted = report.sorted();
    // no-label diagnostics get source_idx=u32::MAX, offset=u32::MAX → sorted last
    assert_eq!(sorted[0].code.as_str(), "SUS-SPAN");
    assert_eq!(sorted[1].code.as_str(), "SUS-NO-SPAN");
}
