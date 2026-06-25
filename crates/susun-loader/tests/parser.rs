//! Integration tests for the Compose YAML parser adapter.

use std::{error::Error, sync::Arc};

use susun_diagnostics::DiagnosticReport;
use susun_loader::parse_compose_str;
use susun_source::{LoadedSource, SourceMap, SourceName};

type TestResult = Result<(), Box<dyn Error>>;

fn register(map: &mut SourceMap, name: &str, contents: &str) -> susun_source::SourceId {
    map.register(LoadedSource {
        name: SourceName::new(name),
        path: None,
        contents: Arc::from(contents),
    })
}

// ── Minimal valid file ────────────────────────────────────────────────────────

#[test]
fn minimal_file_yields_name_and_service() -> TestResult {
    let src = "name: myapp\nservices:\n  web:\n    image: nginx:latest\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;

    assert!(!report.has_errors(), "no errors on valid file");
    let name = project.name.ok_or("expected name field")?;
    assert_eq!(name.value, "myapp");
    assert_eq!(project.services.len(), 1);
    let web = project.services.get("web").ok_or("expected web service")?;
    let image = web.value.image.as_ref().ok_or("expected image field")?;
    assert_eq!(image.value, "nginx:latest");
    Ok(())
}

#[test]
fn name_only_file_parses_without_services() -> TestResult {
    let src = "name: bare\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;

    assert!(!report.has_errors());
    let name = project.name.ok_or("expected name")?;
    assert_eq!(name.value, "bare");
    assert!(project.services.is_empty());
    Ok(())
}

// ── Malformed YAML ────────────────────────────────────────────────────────────

#[test]
fn malformed_yaml_returns_none_with_error_diagnostic() {
    let src = "key: : bad\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "bad.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report);

    assert!(project.is_none(), "malformed YAML yields None");
    assert!(report.has_errors(), "error diagnostic emitted");
    let codes: Vec<&str> = report.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"SUS-PARSE-001"),
        "code SUS-PARSE-001 present"
    );
}

// ── Unknown top-level field ───────────────────────────────────────────────────

#[test]
fn unknown_top_level_field_emits_warning() -> TestResult {
    let src = "name: app\nfoo: bar\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;

    // Project still parsed despite the warning
    assert_eq!(project.name.as_ref().map(|n| n.value.as_str()), Some("app"));
    // A SUS-PARSE-011 warning is present
    let has_warning = report.iter().any(|d| d.code.as_str() == "SUS-PARSE-011");
    assert!(has_warning, "SUS-PARSE-011 warning for unknown field");
    // No errors — warning only
    assert!(!report.has_errors());
    Ok(())
}

// ── Extension fields ──────────────────────────────────────────────────────────

#[test]
fn extension_fields_are_accepted_silently() -> TestResult {
    let src = "name: app\nx-custom: some-data\nx-deploy-config:\n  replicas: 2\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;

    assert!(!report.has_errors(), "no errors for x- fields");
    assert_eq!(report.len(), 0, "no diagnostics at all for x- fields");
    assert_eq!(project.name.as_ref().map(|n| n.value.as_str()), Some("app"));
    Ok(())
}

// ── Multiple documents ────────────────────────────────────────────────────────

#[test]
fn multiple_documents_returns_none_with_error_diagnostic() {
    let src = "name: first\n---\nname: second\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "multi.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report);

    assert!(project.is_none(), "multiple docs yields None");
    assert!(report.has_errors(), "error diagnostic emitted");
    let codes: Vec<&str> = report.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"SUS-PARSE-003"),
        "code SUS-PARSE-003 present"
    );
}

// ── Span accuracy ─────────────────────────────────────────────────────────────

#[test]
fn name_span_points_into_source_contents() -> TestResult {
    let src = "name: myapp\n";
    let mut map = SourceMap::new();
    let id = register(&mut map, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let project = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;
    let name = project.name.ok_or("expected name")?;

    let start = name.span.start.value() as usize;
    let end = name.span.end.value() as usize;
    assert!(end <= src.len(), "span end within source");
    assert_eq!(&src[start..end], "myapp", "span covers value text");
    Ok(())
}
