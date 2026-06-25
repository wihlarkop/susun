#![allow(missing_docs)]

use std::{error::Error, path::PathBuf};

use susun::{Analyzer, Error as SusunError};

type TestResult = Result<(), Box<dyn Error>>;

fn valid_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/valid-minimal/compose.yaml")
}

fn malformed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/malformed/compose.yaml")
}

#[test]
fn valid_file_produces_canonical_project() -> TestResult {
    let result = Analyzer::new(valid_path()).analyze()?;
    let project = result.project.ok_or("expected a project")?;
    assert_eq!(project.name.to_string(), "valid-minimal");
    let key = project
        .services
        .keys()
        .next()
        .ok_or("expected at least one service")?;
    assert_eq!(key.as_str(), "web");
    let service = project
        .services
        .values()
        .next()
        .ok_or("expected service value")?;
    let image = service.image.as_ref().ok_or("expected image")?;
    assert_eq!(image.as_str(), "nginx:latest");
    Ok(())
}

#[test]
fn malformed_file_returns_ok_with_error_report() -> TestResult {
    let result = Analyzer::new(malformed_path()).analyze()?;
    assert!(result.report.has_errors(), "expected error diagnostics");
    assert!(result.project.is_none());
    Ok(())
}

#[test]
fn missing_file_returns_load_error() {
    let err = Analyzer::new("/nonexistent/compose.yaml").analyze().err();
    assert!(matches!(err, Some(SusunError::Load(_))));
}

#[test]
fn valid_file_report_is_clean() -> TestResult {
    let result = Analyzer::new(valid_path()).analyze()?;
    assert!(!result.report.has_errors());
    assert!(result.report.is_empty(), "expected no diagnostics at all");
    Ok(())
}
