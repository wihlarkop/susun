//! Tests for FileSystemSourceProvider and MemorySourceProvider.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{io::Write, path::PathBuf};

use susun_source::{
    LoadLimits, MemorySourceProvider, ProviderError, SourceProvider, SourceRequest,
};

fn req(path: &str) -> SourceRequest {
    SourceRequest::new(PathBuf::from(path))
}

// ── MemorySourceProvider ──────────────────────────────────────────────────────

#[test]
fn memory_provider_returns_exact_contents() {
    let provider = MemorySourceProvider::with_files([("compose.yaml", "name: app\n")]);
    let source = provider.read(&req("compose.yaml")).unwrap();
    assert_eq!(source.contents.as_ref(), "name: app\n");
}

#[test]
fn memory_provider_missing_file_returns_not_found() {
    let provider = MemorySourceProvider::with_files::<_, &str, &str>([]);
    let err = provider.read(&req("missing.yaml")).unwrap_err();
    assert!(matches!(err, ProviderError::NotFound(_)));
}

#[test]
fn memory_provider_enforces_file_size_limit() {
    let big = "x".repeat(100);
    let limits = LoadLimits { max_file_bytes: 10, max_file_count: 10 };
    let provider = MemorySourceProvider::new(
        [("big.yaml".into(), big.as_str().into())].into_iter().collect(),
        limits,
    );
    let err = provider.read(&req("big.yaml")).unwrap_err();
    assert!(matches!(err, ProviderError::FileTooLarge { .. }));
}

#[test]
fn memory_provider_enforces_file_count_limit() {
    let limits = LoadLimits { max_file_bytes: 1024, max_file_count: 2 };
    let provider = MemorySourceProvider::new(
        [
            ("a.yaml".into(), "a".into()),
            ("b.yaml".into(), "b".into()),
            ("c.yaml".into(), "c".into()),
        ]
        .into_iter()
        .collect(),
        limits,
    );
    assert!(provider.read(&req("a.yaml")).is_ok());
    assert!(provider.read(&req("b.yaml")).is_ok());
    let err = provider.read(&req("c.yaml")).unwrap_err();
    assert!(matches!(err, ProviderError::FileCountExceeded { limit: 2 }));
}

#[test]
fn memory_provider_does_not_allocate_source_ids() {
    // SourceProvider::read returns LoadedSource; SourceId allocation is the
    // caller's responsibility via SourceMap::register. Verified structurally:
    // the return type of read() is LoadedSource, not (SourceId, LoadedSource).
    let provider = MemorySourceProvider::with_files([("f.yaml", "data")]);
    let source = provider.read(&req("f.yaml")).unwrap();
    // path is recorded on the returned LoadedSource
    assert_eq!(source.path, Some(PathBuf::from("f.yaml")));
}

// ── FileSystemSourceProvider ──────────────────────────────────────────────────

#[test]
fn fs_provider_missing_file_returns_not_found() {
    let provider = susun_source::FileSystemSourceProvider::with_default_limits();
    let err = provider.read(&req("nonexistent_file_susun_test.yaml")).unwrap_err();
    assert!(matches!(err, ProviderError::NotFound(_)));
}

#[test]
fn fs_provider_reads_real_file() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "name: test").unwrap();
    let path = tmp.path().to_path_buf();
    let provider = susun_source::FileSystemSourceProvider::with_default_limits();
    let source = provider.read(&SourceRequest::new(path)).unwrap();
    assert_eq!(source.contents.as_ref(), "name: test\n");
}

#[test]
fn fs_provider_enforces_file_size_limit() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{}", "x".repeat(20)).unwrap(); // no trailing newline — intentional
    let path = tmp.path().to_path_buf();
    let limits = LoadLimits { max_file_bytes: 10, max_file_count: 10 };
    let provider = susun_source::FileSystemSourceProvider::new(limits);
    let err = provider.read(&SourceRequest::new(path)).unwrap_err();
    assert!(matches!(err, ProviderError::FileTooLarge { .. }));
}
