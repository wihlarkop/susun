//! Integration tests for SourceMap, Span, and related types.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use susun_source::{
    LineColumn, LoadedSource, SourceError, SourceMap, SourceName, Span, TextOffset,
};

fn make_source(name: &str, contents: &str) -> LoadedSource {
    LoadedSource {
        name: SourceName::new(name),
        path: None,
        contents: Arc::from(contents),
    }
}

#[test]
fn opaque_id_allocation_produces_distinct_ids() {
    let mut map = SourceMap::new();
    let id1 = map.register(make_source("a", "hello"));
    let id2 = map.register(make_source("b", "world"));
    assert_ne!(id1, id2);
}

#[test]
fn registered_source_is_retrievable() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("myfile.yaml", "content"));
    let source = map.get(id).expect("registered source must be retrievable");
    assert_eq!(source.name.as_ref(), "myfile.yaml");
}

#[test]
fn id_from_other_map_is_unknown() {
    let mut map2 = SourceMap::new();
    let id = map2.register(make_source("x", "y"));
    let map = SourceMap::new();
    assert!(map.get(id).is_none());
}

#[test]
fn resolve_unknown_id_returns_error() {
    let mut map2 = SourceMap::new();
    let id = map2.register(make_source("x", "y"));
    let map = SourceMap::new();
    let err = map.resolve(id, TextOffset::new(0));
    assert!(matches!(err, Err(SourceError::UnknownSourceId(_))));
}

#[test]
fn span_start_after_end_is_error() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello"));
    let result = Span::new(id, TextOffset::new(3), TextOffset::new(1));
    assert!(matches!(
        result,
        Err(SourceError::SpanStartAfterEnd { start: 3, end: 1 })
    ));
}

#[test]
fn span_equal_start_end_is_ok() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello"));
    assert!(Span::new(id, TextOffset::new(2), TextOffset::new(2)).is_ok());
}

#[test]
fn offset_out_of_bounds_returns_error() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello")); // len = 5
    let err = map.resolve(id, TextOffset::new(6));
    assert!(matches!(
        err,
        Err(SourceError::OffsetOutOfBounds { offset: 6, len: 5 })
    ));
}

#[test]
fn offset_at_end_is_valid() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello")); // len = 5
    assert!(map.resolve(id, TextOffset::new(5)).is_ok());
}

#[test]
fn utf8_boundary_violation_returns_error() {
    let mut map = SourceMap::new();
    // 'é' = U+00E9 → 2 bytes (0xC3 0xA9)
    // "café": c@0, a@1, f@2, é@3..5  (5 bytes total)
    let id = map.register(make_source("src", "café"));
    let err = map.resolve(id, TextOffset::new(4));
    assert!(matches!(err, Err(SourceError::NotUtf8Boundary { offset: 4 })));
}

#[test]
fn line_column_first_char_is_line1_col1() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello\nworld\n"));
    let lc = map.resolve(id, TextOffset::new(0)).unwrap();
    assert_eq!(lc, LineColumn { line: 1, column: 1 });
}

#[test]
fn line_column_second_line_start() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello\nworld\n"));
    // "world" starts at offset 6 (after "hello\n")
    let lc = map.resolve(id, TextOffset::new(6)).unwrap();
    assert_eq!(lc, LineColumn { line: 2, column: 1 });
}

#[test]
fn line_column_mid_line() {
    let mut map = SourceMap::new();
    let id = map.register(make_source("src", "hello\nworld\n"));
    // 'o' in "world" at offset 7
    let lc = map.resolve(id, TextOffset::new(7)).unwrap();
    assert_eq!(lc, LineColumn { line: 2, column: 2 });
}

#[test]
fn line_column_unicode_multibyte_char() {
    let mut map = SourceMap::new();
    // "café\nx\n": c@0, a@1, f@2, é@3..5, \n@5, x@6, \n@7
    let id = map.register(make_source("src", "café\nx\n"));
    let lc = map.resolve(id, TextOffset::new(6)).unwrap();
    assert_eq!(lc, LineColumn { line: 2, column: 1 });
}

#[test]
fn source_name_display_and_as_ref() {
    let name = SourceName::new("compose.yaml");
    assert_eq!(name.to_string(), "compose.yaml");
    assert_eq!(name.as_ref(), "compose.yaml");
}
