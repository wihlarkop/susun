#![allow(missing_docs)]

use std::{collections::BTreeMap, error::Error, path::PathBuf};

use susun_diagnostics::{DiagnosticReport, Severity};
use susun_loader::{
    EnvResolver, MapEnvironment,
    interpolation::{eval::interpolate, parser::{Token, parse}},
};
use susun_source::{MemorySourceProvider, SourceMap, SourceProvider, SourceRequest, Span, TextOffset};

type TestResult = Result<(), Box<dyn Error>>;

// ── helpers ──────────────────────────────────────────────────────────────────

fn dummy_span() -> Result<Span, Box<dyn Error>> {
    let path = PathBuf::from("compose.yaml");
    let provider = MemorySourceProvider::with_files([(path.clone(), "dummy")]);
    let loaded = provider.read(&SourceRequest::new(&path))?;
    let mut sm = SourceMap::new();
    let source_id = sm.register(loaded);
    Ok(Span::empty(source_id, TextOffset::new(0)))
}

fn make_resolver(vars: &[(&str, &str)]) -> EnvResolver {
    let map: BTreeMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    EnvResolver::new(MapEnvironment::new(map), vec![], vec![])
}

fn eval(input: &str, vars: &[(&str, &str)]) -> Result<(String, DiagnosticReport), Box<dyn Error>> {
    let span = dummy_span()?;
    let resolver = make_resolver(vars);
    let mut report = DiagnosticReport::new();
    let output = interpolate(input, &resolver, span, &mut report);
    Ok((output, report))
}

// ── parser unit tests ─────────────────────────────────────────────────────────

#[test]
fn parser_empty_string_produces_no_tokens() {
    assert_eq!(parse(""), vec![]);
}

#[test]
fn parser_plain_string_without_dollar_is_one_literal() {
    assert_eq!(parse("hello world"), vec![Token::Literal("hello world")]);
}

#[test]
fn parser_escaped_dollar_alone() {
    assert_eq!(parse("$$"), vec![Token::EscapedDollar]);
}

#[test]
fn parser_escaped_dollar_followed_by_text() {
    assert_eq!(
        parse("$$VAR"),
        vec![Token::EscapedDollar, Token::Literal("VAR")],
    );
}

#[test]
fn parser_simple_substitution() {
    assert_eq!(parse("${VAR}"), vec![Token::Substitute { name: "VAR" }]);
}

#[test]
fn parser_default_check_empty_true() {
    assert_eq!(
        parse("${VAR:-default}"),
        vec![Token::WithDefault { name: "VAR", check_empty: true, default: "default" }],
    );
}

#[test]
fn parser_default_check_empty_false() {
    assert_eq!(
        parse("${VAR-default}"),
        vec![Token::WithDefault { name: "VAR", check_empty: false, default: "default" }],
    );
}

#[test]
fn parser_required_check_empty_true() {
    assert_eq!(
        parse("${VAR:?my message}"),
        vec![Token::Required { name: "VAR", check_empty: true, message: "my message" }],
    );
}

#[test]
fn parser_required_check_empty_false() {
    assert_eq!(
        parse("${VAR?my message}"),
        vec![Token::Required { name: "VAR", check_empty: false, message: "my message" }],
    );
}

#[test]
fn parser_bare_dollar_before_identifier_is_literal() {
    // $VAR without braces → literal "$VAR" (braces required)
    assert_eq!(parse("$VAR"), vec![Token::Literal("$VAR")]);
}

#[test]
fn parser_lone_dollar_at_end_is_literal() {
    // Lone '$' at end — emitted as a separate Literal segment alongside any preceding text.
    assert_eq!(parse("end$"), vec![Token::Literal("end"), Token::Literal("$")]);
}

#[test]
fn parser_dollar_only_is_literal() {
    assert_eq!(parse("$"), vec![Token::Literal("$")]);
}

#[test]
fn parser_adjacent_expressions() {
    assert_eq!(
        parse("${A}${B}"),
        vec![Token::Substitute { name: "A" }, Token::Substitute { name: "B" }],
    );
}

#[test]
fn parser_literal_then_subst_then_literal() {
    assert_eq!(
        parse("prefix_${VAR}_suffix"),
        vec![
            Token::Literal("prefix_"),
            Token::Substitute { name: "VAR" },
            Token::Literal("_suffix"),
        ],
    );
}

#[test]
fn parser_unmatched_brace_at_end() {
    assert_eq!(parse("${VAR"), vec![Token::UnmatchedBrace { content: "VAR" }]);
}

#[test]
fn parser_empty_braces_is_invalid_expr() {
    assert_eq!(parse("${}"), vec![Token::InvalidExpr { content: "" }]);
}

#[test]
fn parser_numeric_name_is_invalid_expr() {
    assert_eq!(parse("${123}"), vec![Token::InvalidExpr { content: "123" }]);
}

#[test]
fn parser_empty_default_value() {
    // ${VAR:-} — default is empty string
    assert_eq!(
        parse("${VAR:-}"),
        vec![Token::WithDefault { name: "VAR", check_empty: true, default: "" }],
    );
}

#[test]
fn parser_empty_required_message() {
    assert_eq!(
        parse("${VAR:?}"),
        vec![Token::Required { name: "VAR", check_empty: true, message: "" }],
    );
}

#[test]
fn parser_underscore_prefixed_name() {
    assert_eq!(parse("${_INTERNAL}"), vec![Token::Substitute { name: "_INTERNAL" }]);
}

// ── evaluator integration tests ───────────────────────────────────────────────

/// Property: any string without `$` passes through unchanged.
#[test]
fn eval_no_dollar_unchanged() -> TestResult {
    for input in &["", "hello", "foo bar", "42", "true", "/path/to/file"] {
        let (out, report) = eval(input, &[])?;
        assert_eq!(&out, input, "input: {input}");
        assert!(report.is_empty());
    }
    Ok(())
}

/// Property: `$$X` → `$X`.
#[test]
fn eval_escaped_dollar_produces_literal_dollar() -> TestResult {
    let cases: &[(&str, &str)] = &[
        ("$$", "$"),
        ("$$VAR", "$VAR"),
        ("$$FOO$$BAR", "$FOO$BAR"),
        ("prefix$$suffix", "prefix$suffix"),
    ];
    for (input, expected) in cases {
        let (out, report) = eval(input, &[])?;
        assert_eq!(&out, expected, "input: {input}");
        assert!(report.is_empty());
    }
    Ok(())
}

#[test]
fn eval_simple_subst_set_var() -> TestResult {
    let (out, report) = eval("${PORT}", &[("PORT", "8080")])?;
    assert_eq!(out, "8080");
    assert!(report.is_empty());
    Ok(())
}

#[test]
fn eval_simple_subst_unset_var_is_empty() -> TestResult {
    let (out, report) = eval("${PORT}", &[])?;
    assert_eq!(out, "");
    assert!(report.is_empty());
    Ok(())
}

#[test]
fn eval_with_default_check_empty_true_var_set_non_empty() -> TestResult {
    let (out, _) = eval("${DB:-postgres}", &[("DB", "mysql")])?;
    assert_eq!(out, "mysql");
    Ok(())
}

#[test]
fn eval_with_default_check_empty_true_var_empty_uses_default() -> TestResult {
    let (out, _) = eval("${DB:-postgres}", &[("DB", "")])?;
    assert_eq!(out, "postgres");
    Ok(())
}

#[test]
fn eval_with_default_check_empty_true_var_unset_uses_default() -> TestResult {
    let (out, _) = eval("${DB:-postgres}", &[])?;
    assert_eq!(out, "postgres");
    Ok(())
}

#[test]
fn eval_with_default_check_empty_false_var_empty_keeps_empty() -> TestResult {
    // ${VAR-default} — only unset triggers default; empty string is kept.
    let (out, _) = eval("${DB-postgres}", &[("DB", "")])?;
    assert_eq!(out, "");
    Ok(())
}

#[test]
fn eval_with_default_check_empty_false_var_unset_uses_default() -> TestResult {
    let (out, _) = eval("${DB-postgres}", &[])?;
    assert_eq!(out, "postgres");
    Ok(())
}

#[test]
fn eval_required_colon_q_var_set_non_empty() -> TestResult {
    let (out, report) = eval("${DB:?must be set}", &[("DB", "mydb")])?;
    assert_eq!(out, "mydb");
    assert!(!report.has_errors());
    Ok(())
}

#[test]
fn eval_required_colon_q_var_unset_emits_error() -> TestResult {
    let (out, report) = eval("${DB:?must be set}", &[])?;
    assert_eq!(out, "");
    assert!(report.has_errors());
    let diag = report.iter().next().ok_or("expected one error")?;
    assert_eq!(diag.code.as_str(), "SUS-ENV-001");
    assert_eq!(diag.severity, Severity::Error);
    assert!(diag.message.contains("DB"));
    assert!(diag.message.contains("must be set"));
    Ok(())
}

#[test]
fn eval_required_colon_q_var_empty_emits_error() -> TestResult {
    let (out, report) = eval("${DB:?must be set}", &[("DB", "")])?;
    assert_eq!(out, "");
    assert!(report.has_errors());
    Ok(())
}

#[test]
fn eval_required_q_only_var_unset_emits_error() -> TestResult {
    let (out, report) = eval("${DB?required}", &[])?;
    assert_eq!(out, "");
    assert!(report.has_errors());
    Ok(())
}

#[test]
fn eval_required_q_only_var_empty_is_not_error() -> TestResult {
    // ${VAR?msg} — empty string is acceptable; only unset triggers error.
    let (out, report) = eval("${DB?required}", &[("DB", "")])?;
    assert_eq!(out, "");
    assert!(!report.has_errors());
    Ok(())
}

#[test]
fn eval_required_empty_message_falls_back_to_default_text() -> TestResult {
    let (_out, report) = eval("${DB:?}", &[])?;
    assert!(report.has_errors());
    let diag = report.iter().next().ok_or("expected one error")?;
    assert!(diag.message.contains("DB"));
    Ok(())
}

#[test]
fn eval_unmatched_brace_passes_through() -> TestResult {
    let (out, report) = eval("${VAR", &[])?;
    assert_eq!(out, "${VAR");
    assert!(report.is_empty());
    Ok(())
}

#[test]
fn eval_invalid_expr_passes_through() -> TestResult {
    // ${ with empty name passes through literally
    let (out, report) = eval("${}", &[])?;
    assert_eq!(out, "${}");
    assert!(report.is_empty());
    Ok(())
}

#[test]
fn eval_adjacent_expressions() -> TestResult {
    let (out, _) = eval("${HOST}:${PORT}", &[("HOST", "localhost"), ("PORT", "5432")])?;
    assert_eq!(out, "localhost:5432");
    Ok(())
}

#[test]
fn eval_default_is_not_recursively_interpolated() -> TestResult {
    // The default "${NESTED}" should not be interpreted as an expression.
    let (out, _) = eval("${MISSING:-${NESTED}}", &[("NESTED", "found")])?;
    // Expected: default is "${NESTED}" literally (no recursion)
    assert_eq!(out, "${NESTED}");
    Ok(())
}

#[test]
fn eval_multiple_required_vars_each_emits_separate_error() -> TestResult {
    let (out, report) = eval("${A:?err} ${B:?err}", &[])?;
    assert_eq!(out, " ");
    assert_eq!(report.len(), 2);
    assert!(report.has_errors());
    Ok(())
}

#[test]
fn eval_bare_dollar_is_not_interpolated() -> TestResult {
    // Bare $VAR (no braces) is passed through unchanged.
    let (out, report) = eval("$VAR", &[("VAR", "should not appear")])?;
    assert_eq!(out, "$VAR");
    assert!(report.is_empty());
    Ok(())
}
