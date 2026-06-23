//! Evaluates tokenized interpolation expressions against a resolved environment.

use susun_diagnostics::{Diagnostic, DiagnosticReport, Label, Severity};
use susun_source::Span;

use crate::environment::resolve::EnvResolver;
use super::parser::{Token, parse};

/// Interpolates `${...}` expressions in `input` using `resolver`.
///
/// - Missing or (when `check_empty`) empty required variables (`${VAR:?msg}`,
///   `${VAR?msg}`) produce `SUS-ENV-001` errors attached to `value_span`.
/// - Unrecognized `${...}` structures and unmatched braces are passed through
///   as-is without error.
/// - Substituted values are **not** recursively interpolated.
pub fn interpolate(
    input: &str,
    resolver: &EnvResolver,
    value_span: Span,
    report: &mut DiagnosticReport,
) -> String {
    let tokens = parse(input);
    let mut output = String::with_capacity(input.len());

    for token in tokens {
        match token {
            Token::Literal(s) => output.push_str(s),

            Token::EscapedDollar => output.push('$'),

            Token::Substitute { name } => {
                if let Some(v) = resolver.get(name) {
                    output.push_str(&v);
                }
                // Unset → empty string (Docker Compose behaviour)
            }

            Token::WithDefault { name, check_empty, default } => {
                let value = resolver.get(name);
                let use_default = match &value {
                    None => true,
                    Some(v) if check_empty && v.is_empty() => true,
                    _ => false,
                };
                if use_default {
                    output.push_str(default);
                } else {
                    output.push_str(value.as_deref().unwrap_or(""));
                }
            }

            Token::Required { name, check_empty, message } => {
                let value = resolver.get(name);
                let is_missing = match &value {
                    None => true,
                    Some(v) if check_empty && v.is_empty() => true,
                    _ => false,
                };
                if is_missing {
                    let diag_msg = if message.is_empty() {
                        format!("required variable `{name}` is not set")
                    } else {
                        format!("`{name}` is not set: {message}")
                    };
                    report.push(
                        Diagnostic::new("SUS-ENV-001", Severity::Error, diag_msg)
                            .with_label(Label::primary(value_span, "required here")),
                    );
                } else {
                    output.push_str(value.as_deref().unwrap_or(""));
                }
            }

            // Pass-through: unmatched `${` — reconstruct without closing brace.
            Token::UnmatchedBrace { content } => {
                output.push_str("${");
                output.push_str(content);
            }

            // Pass-through: `${...}` with unknown/invalid operator.
            Token::InvalidExpr { content } => {
                output.push_str("${");
                output.push_str(content);
                output.push('}');
            }
        }
    }

    output
}
