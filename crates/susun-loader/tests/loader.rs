#![allow(missing_docs)]

use std::{error::Error, path::PathBuf};

use susun_loader::{LoadError, ProjectLoader};

type TestResult = Result<(), Box<dyn Error>>;

fn valid_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/valid-minimal/compose.yaml")
}

fn malformed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/malformed/compose.yaml")
}

#[test]
fn valid_file_produces_some_parsed() -> TestResult {
    let result = ProjectLoader::new(valid_path()).load()?;
    assert!(result.parsed.is_some());
    assert!(!result.report.has_errors());
    Ok(())
}

#[test]
fn malformed_file_produces_none_with_error_diagnostic() -> TestResult {
    let result = ProjectLoader::new(malformed_path()).load()?;
    assert!(result.parsed.is_none());
    assert!(result.report.has_errors());
    Ok(())
}

#[test]
fn missing_file_returns_not_found_error() {
    let result = ProjectLoader::new("/nonexistent/compose.yaml").load();
    assert!(matches!(result, Err(LoadError::NotFound { .. })));
}

#[test]
fn source_map_registers_loaded_source() -> TestResult {
    let result = ProjectLoader::new(valid_path()).load()?;
    assert!(result.source_map.get(result.source_id).is_some());
    Ok(())
}
