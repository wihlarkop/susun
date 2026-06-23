use std::{error::Error, path::PathBuf};

use susun_diagnostics::DiagnosticReport;
use susun_loader::{
    DotenvEntry,
    environment::dotenv::parse_dotenv,
};
use susun_source::{
    MemorySourceProvider, SourceId, SourceMap, SourceProvider, SourceRequest,
};

type TestResult = Result<(), Box<dyn Error>>;

/// Register `contents` as ".env" in a fresh source map, return (source_id, source_map).
fn register(contents: &str) -> TestResult2 {
    let path = PathBuf::from(".env");
    let provider = MemorySourceProvider::with_files([(path.clone(), contents)]);
    let loaded = provider.read(&SourceRequest::new(&path))?;
    let mut source_map = SourceMap::new();
    let source_id = source_map.register(loaded);
    Ok((source_id, source_map))
}

type TestResult2 = Result<(SourceId, SourceMap), Box<dyn Error>>;

fn parse(contents: &str) -> Result<(Vec<DotenvEntry>, DiagnosticReport, SourceMap), Box<dyn Error>> {
    let (source_id, source_map) = register(contents)?;
    let mut report = DiagnosticReport::new();
    let entries = parse_dotenv(source_id, contents, &mut report);
    Ok((entries, report, source_map))
}

fn keys(entries: &[DotenvEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.key.as_str()).collect()
}

fn get<'a>(entries: &'a [DotenvEntry], key: &str) -> Option<&'a str> {
    entries.iter().find(|e| e.key == key).map(|e| e.value.as_str())
}

// --- Comments and blank lines ---

#[test]
fn comments_and_blanks_are_skipped() -> TestResult {
    let (entries, report, _) = parse("# comment\n\nFOO=bar\n")?;
    assert!(!report.has_errors());
    assert_eq!(keys(&entries), vec!["FOO"]);
    assert_eq!(get(&entries, "FOO"), Some("bar"));
    Ok(())
}

#[test]
fn inline_comment_is_part_of_unquoted_value() -> TestResult {
    // Docker Compose does NOT strip inline comments from unquoted values.
    let (entries, report, _) = parse("FOO=bar # not a comment\n")?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "FOO"), Some("bar # not a comment"));
    Ok(())
}

// --- Unquoted values ---

#[test]
fn unquoted_value_is_verbatim() -> TestResult {
    let (entries, _, _) = parse("KEY=hello world\n")?;
    assert_eq!(get(&entries, "KEY"), Some("hello world"));
    Ok(())
}

// --- Double-quoted values ---

#[test]
fn double_quoted_value_strips_quotes() -> TestResult {
    let (entries, _, _) = parse("KEY=\"double quoted\"\n")?;
    assert_eq!(get(&entries, "KEY"), Some("double quoted"));
    Ok(())
}

#[test]
fn double_quoted_empty_value() -> TestResult {
    let (entries, _, _) = parse("KEY=\"\"\n")?;
    assert_eq!(get(&entries, "KEY"), Some(""));
    Ok(())
}

#[test]
fn double_quoted_escape_sequences() -> TestResult {
    let (entries, _, _) = parse("KEY=\"a\\nb\\tc\"\n")?;
    assert_eq!(get(&entries, "KEY"), Some("a\nb\tc"));
    Ok(())
}

#[test]
fn double_quoted_escaped_quote() -> TestResult {
    let (entries, _, _) = parse("KEY=\"say \\\"hi\\\"\"\n")?;
    assert_eq!(get(&entries, "KEY"), Some("say \"hi\""));
    Ok(())
}

// --- Single-quoted values ---

#[test]
fn single_quoted_value_is_literal() -> TestResult {
    let (entries, _, _) = parse("KEY='literal \\n no escape'\n")?;
    assert_eq!(get(&entries, "KEY"), Some("literal \\n no escape"));
    Ok(())
}

#[test]
fn single_quoted_empty_value() -> TestResult {
    let (entries, _, _) = parse("KEY=''\n")?;
    assert_eq!(get(&entries, "KEY"), Some(""));
    Ok(())
}

// --- Empty values ---

#[test]
fn equals_with_no_value_is_empty_string() -> TestResult {
    let (entries, report, _) = parse("KEY=\n")?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "KEY"), Some(""));
    Ok(())
}

// --- Bare keys ---

#[test]
fn bare_key_without_equals_is_empty_value() -> TestResult {
    let (entries, report, _) = parse("BARE_KEY\n")?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "BARE_KEY"), Some(""));
    Ok(())
}

// --- export prefix ---

#[test]
fn export_prefix_is_stripped() -> TestResult {
    let (entries, report, _) = parse("export MY_VAR=hello\n")?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "MY_VAR"), Some("hello"));
    Ok(())
}

// --- CRLF ---

#[test]
fn crlf_line_endings_are_supported() -> TestResult {
    let (entries, report, _) = parse("FOO=bar\r\nBAZ=qux\r\n")?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "FOO"), Some("bar"));
    assert_eq!(get(&entries, "BAZ"), Some("qux"));
    Ok(())
}

// --- Duplicate keys ---

#[test]
fn duplicate_key_emits_warning_and_last_value_wins() -> TestResult {
    let (entries, report, _) = parse("KEY=first\nKEY=second\n")?;
    assert!(!report.has_errors());
    assert_eq!(report.len(), 1, "expected exactly one warning");
    assert_eq!(get(&entries, "KEY"), Some("second"));
    let diag_codes: Vec<&str> = report.sorted().iter().map(|d| d.code.as_str()).collect();
    assert!(diag_codes.contains(&"SUS-ENV-003"));
    Ok(())
}

#[test]
fn duplicate_key_entry_appears_once_in_output() -> TestResult {
    let (entries, _, _) = parse("A=1\nB=2\nA=3\n")?;
    assert_eq!(entries.len(), 2, "A should appear only once");
    assert_eq!(get(&entries, "A"), Some("3"));
    assert_eq!(get(&entries, "B"), Some("2"));
    Ok(())
}

// --- Invalid identifiers ---

#[test]
fn invalid_key_starting_with_digit_emits_error() -> TestResult {
    let (entries, report, _) = parse("123BAD=value\n")?;
    assert!(report.has_errors(), "expected error for invalid key");
    assert!(entries.is_empty(), "invalid key should be skipped");
    let diag_codes: Vec<&str> = report.sorted().iter().map(|d| d.code.as_str()).collect();
    assert!(diag_codes.contains(&"SUS-ENV-002"));
    Ok(())
}

#[test]
fn invalid_key_with_hyphen_emits_error() -> TestResult {
    let (entries, report, _) = parse("MY-VAR=value\n")?;
    assert!(report.has_errors());
    assert!(entries.is_empty());
    Ok(())
}

#[test]
fn empty_key_emits_error() -> TestResult {
    let (entries, report, _) = parse("=value\n")?;
    assert!(report.has_errors());
    assert!(entries.is_empty());
    Ok(())
}

#[test]
fn valid_identifiers_include_leading_underscore_and_digits() -> TestResult {
    let (entries, report, _) = parse("_PRIV=a\nVAR2=b\n")?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "_PRIV"), Some("a"));
    assert_eq!(get(&entries, "VAR2"), Some("b"));
    Ok(())
}

// --- Fixture files ---

#[test]
fn basic_fixture_parses_without_errors() -> TestResult {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/interpolation/dotenv/basic.env");
    let contents = std::fs::read_to_string(&fixture)?;
    let (entries, report, _) = parse(&contents)?;
    assert!(!report.has_errors());
    assert_eq!(get(&entries, "KEY"), Some("value"));
    assert_eq!(get(&entries, "QUOTED_DOUBLE"), Some("double quoted"));
    assert_eq!(get(&entries, "QUOTED_SINGLE"), Some("single quoted"));
    assert_eq!(get(&entries, "EMPTY"), Some(""));
    assert_eq!(get(&entries, "BARE_KEY"), Some(""));
    Ok(())
}

#[test]
fn duplicates_fixture_emits_warning() -> TestResult {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/interpolation/dotenv/duplicates.env");
    let contents = std::fs::read_to_string(&fixture)?;
    let (entries, report, _) = parse(&contents)?;
    assert!(!report.has_errors());
    assert_eq!(report.len(), 1);
    assert_eq!(get(&entries, "KEY"), Some("second"));
    assert_eq!(get(&entries, "OTHER"), Some("value"));
    Ok(())
}

// --- Span accuracy ---

#[test]
fn key_span_covers_key_bytes() -> TestResult {
    let contents = "MYKEY=value\n";
    let (source_id, source_map) = register(contents)?;
    let mut report = DiagnosticReport::new();
    let entries = parse_dotenv(source_id, contents, &mut report);

    let entry = entries.first().ok_or("no entries")?;
    assert_eq!(entry.key, "MYKEY");

    // The span start should point to 'M' (byte 0), end at byte 5.
    assert_eq!(entry.key_span.start.value(), 0);
    assert_eq!(entry.key_span.end.value(), 5);

    // Verify the source map can resolve to line 1, column 1.
    let lc = source_map.resolve(source_id, entry.key_span.start)?;
    assert_eq!(lc.line, 1);
    assert_eq!(lc.column, 1);
    Ok(())
}

#[test]
fn key_span_on_second_line_has_correct_offset() -> TestResult {
    let contents = "FIRST=a\nSECOND=b\n";
    let (source_id, source_map) = register(contents)?;
    let mut report = DiagnosticReport::new();
    let entries = parse_dotenv(source_id, contents, &mut report);

    let entry = entries.iter().find(|e| e.key == "SECOND").ok_or("SECOND not found")?;
    // "FIRST=a\n" is 8 bytes; "SECOND" starts at byte 8.
    assert_eq!(entry.key_span.start.value(), 8);
    assert_eq!(entry.key_span.end.value(), 14);

    let lc = source_map.resolve(source_id, entry.key_span.start)?;
    assert_eq!(lc.line, 2);
    assert_eq!(lc.column, 1);
    Ok(())
}
