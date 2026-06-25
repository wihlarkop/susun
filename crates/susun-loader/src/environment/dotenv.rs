//! `.env` file parser with source-aware diagnostics.
//!
//! Supports the Docker Compose `.env` subset: comments, blank lines, `KEY=VALUE`,
//! double/single-quoted values, bare keys, `export KEY=VALUE`, and CRLF endings.

use std::collections::HashMap;

use susun_diagnostics::{Diagnostic, DiagnosticReport, Label, Severity};
use susun_source::{SourceId, Span, TextOffset};

const ENV_INVALID_KEY: &str = "SUS-ENV-002";
const ENV_DUPLICATE_KEY: &str = "SUS-ENV-003";

/// An entry parsed from a `.env` file.
#[derive(Debug, Clone)]
pub struct DotenvEntry {
    /// Variable name.
    pub key: String,
    /// Variable value. Empty string for bare keys and `KEY=`.
    pub value: String,
    /// Source span of the key in the `.env` file.
    pub key_span: Span,
}

/// Parses a `.env` file, appending diagnostics for recoverable errors.
///
/// Supported format:
///
/// | Form | Meaning |
/// |------|---------|
/// | `# comment` | Ignored line |
/// | `KEY=VALUE` | Unquoted value — verbatim |
/// | `KEY="VALUE"` | Double-quoted — `\\`, `\"`, `\n`, `\t`, `\r` unescaped |
/// | `KEY='VALUE'` | Single-quoted — literal, no escaping |
/// | `KEY=` | Empty value |
/// | `KEY` | Bare key — treated as empty value |
/// | `export KEY=VALUE` | `export ` prefix stripped |
/// | CRLF line endings | Supported |
///
/// Diagnostics emitted:
/// - `SUS-ENV-002`: key contains characters outside `[A-Za-z_][A-Za-z0-9_]*` (error, skipped)
/// - `SUS-ENV-003`: duplicate key in the same file (warning, last value wins)
pub fn parse_dotenv(
    source_id: SourceId,
    contents: &str,
    report: &mut DiagnosticReport,
) -> Vec<DotenvEntry> {
    let mut entries: Vec<DotenvEntry> = Vec::new();
    // Maps key name -> index in `entries` for duplicate detection.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut byte_pos: u32 = 0;

    for raw_line in contents.split('\n') {
        let line_start = byte_pos;
        // Advance past this raw line plus its '\n'. The last line may lack a
        // trailing '\n', but `line_start` is already correct.
        byte_pos += raw_line.len() as u32 + 1;

        // Strip trailing '\r' (CRLF support).
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        // Skip blank lines.
        if line.trim().is_empty() {
            continue;
        }

        // Skip comment lines (first non-whitespace char is '#').
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }

        // Byte offset of `trimmed` within `contents` (leading whitespace removed).
        let trimmed_start = line_start + (line.len() - trimmed.len()) as u32;

        // Strip optional `export ` prefix (with one or more trailing spaces).
        let (effective, extra_offset) = if let Some(after_export) = trimmed.strip_prefix("export ")
        {
            let stripped = after_export.trim_start();
            let skip = (trimmed.len() - stripped.len()) as u32;
            (stripped, skip)
        } else {
            (trimmed, 0u32)
        };

        let key_abs_start = trimmed_start + extra_offset;

        // Split on the first '=' to separate key from value.
        let (key_raw, value_raw) = match effective.find('=') {
            Some(eq_pos) => (&effective[..eq_pos], Some(&effective[eq_pos + 1..])),
            None => (effective, None),
        };

        let key_end = key_abs_start + key_raw.len() as u32;
        let key_span = make_span(source_id, key_abs_start, key_end);

        if !is_valid_identifier(key_raw) {
            report.push(
                Diagnostic::new(
                    ENV_INVALID_KEY,
                    Severity::Error,
                    format!("invalid .env key `{key_raw}`"),
                )
                .with_label(Label::primary(key_span, "expected [A-Za-z_][A-Za-z0-9_]*")),
            );
            continue;
        }

        let value = value_raw.map(parse_value).unwrap_or_default();

        if let Some(&prev_idx) = seen.get(key_raw) {
            let prev_span = entries[prev_idx].key_span;
            report.push(
                Diagnostic::new(
                    ENV_DUPLICATE_KEY,
                    Severity::Warning,
                    format!("duplicate key `{key_raw}` in .env file, last value wins"),
                )
                .with_label(Label::primary(key_span, "redefined here"))
                .with_label(Label::secondary(prev_span, "first defined here")),
            );
            entries[prev_idx].value = value;
        } else {
            seen.insert(key_raw.to_owned(), entries.len());
            entries.push(DotenvEntry {
                key: key_raw.to_owned(),
                value,
                key_span,
            });
        }
    }

    entries
}

fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_value(raw: &str) -> String {
    if let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        unescape_double_quoted(inner)
    } else if let Some(inner) = raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        inner.to_owned()
    } else {
        raw.to_owned()
    }
}

fn unescape_double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn make_span(source_id: SourceId, start: u32, end: u32) -> Span {
    Span::new(source_id, TextOffset::new(start), TextOffset::new(end))
        .unwrap_or_else(|_| Span::empty(source_id, TextOffset::new(start)))
}
