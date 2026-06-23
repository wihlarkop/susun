//! Tokenizer for Compose scalar interpolation expressions.
//!
//! Produces a sequence of [`Token`]s from an input string; the caller drives
//! evaluation via `eval::interpolate`.

/// A single segment of an interpolation expression.
#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    /// Literal text that passes through unchanged.
    Literal(&'a str),
    /// `$$` — expands to a single `$`.
    EscapedDollar,
    /// `${VAR}` — substitute value of `VAR` or empty string if unset.
    Substitute {
        /// Variable name.
        name: &'a str,
    },
    /// `${VAR:-default}` or `${VAR-default}`.
    ///
    /// When `check_empty` is `true` (`:-`), an empty-string value also
    /// triggers the default. When `false` (`-`), only unset does.
    WithDefault {
        /// Variable name.
        name: &'a str,
        /// Whether to use the default for an empty-string value.
        check_empty: bool,
        /// Default text (not recursively interpolated).
        default: &'a str,
    },
    /// `${VAR:?message}` or `${VAR?message}`.
    ///
    /// Produces a `SUS-ENV-001` diagnostic when `VAR` is missing or (if
    /// `check_empty` is `true`) empty.
    Required {
        /// Variable name.
        name: &'a str,
        /// Whether an empty-string value also triggers the error.
        check_empty: bool,
        /// Custom error message (may be empty).
        message: &'a str,
    },
    /// `${` with no matching `}`. Content is everything after `${`.
    UnmatchedBrace {
        /// Text following `${` to end of string.
        content: &'a str,
    },
    /// `${...}` with an unrecognized or invalid structure, passed through.
    InvalidExpr {
        /// Text inside `${...}`.
        content: &'a str,
    },
}

/// Tokenizes `input` into a sequence of [`Token`]s.
///
/// The entire `input` is covered by the returned tokens — no bytes are dropped.
pub fn parse(input: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let len = input.len();
    let mut pos = 0usize;
    let mut literal_start = 0usize;

    while pos < len {
        if bytes[pos] != b'$' {
            pos += 1;
            continue;
        }

        // Flush any preceding literal segment.
        if pos > literal_start {
            tokens.push(Token::Literal(&input[literal_start..pos]));
        }

        let dollar_pos = pos;
        pos += 1; // advance past '$'

        if pos >= len {
            // Lone '$' at the end of input — pass through as literal.
            tokens.push(Token::Literal(&input[dollar_pos..pos]));
            literal_start = pos;
            break;
        }

        match bytes[pos] {
            b'$' => {
                tokens.push(Token::EscapedDollar);
                pos += 1;
                literal_start = pos;
            }
            b'{' => {
                pos += 1; // advance past '{'
                let content_start = pos;
                // Find matching '}'.
                while pos < len && bytes[pos] != b'}' {
                    pos += 1;
                }
                if pos >= len {
                    tokens.push(Token::UnmatchedBrace { content: &input[content_start..] });
                    literal_start = pos;
                } else {
                    let content = &input[content_start..pos];
                    pos += 1; // advance past '}'
                    tokens.push(parse_braced(content));
                    literal_start = pos;
                }
            }
            _ => {
                // Bare '$' followed by a non-special char — include '$' in the
                // next literal segment by resetting literal_start to dollar_pos.
                literal_start = dollar_pos;
                // Do NOT advance pos — let the loop handle the current char.
            }
        }
    }

    // Flush any remaining literal.
    if literal_start < len {
        tokens.push(Token::Literal(&input[literal_start..]));
    }

    tokens
}

/// Parses the content inside `${...}`.
fn parse_braced(content: &str) -> Token<'_> {
    // Scan the variable name: [A-Za-z_][A-Za-z0-9_]*
    let name_end = content
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(content.len());

    let name = &content[..name_end];
    let rest = &content[name_end..];

    // Reject empty names or names that don't start with a letter or '_'.
    let first_valid = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
    if name.is_empty() || !first_valid {
        return Token::InvalidExpr { content };
    }

    match rest {
        "" => Token::Substitute { name },
        s if s.starts_with(":-") => Token::WithDefault { name, check_empty: true, default: &s[2..] },
        s if s.starts_with('-') => Token::WithDefault { name, check_empty: false, default: &s[1..] },
        s if s.starts_with(":?") => Token::Required { name, check_empty: true, message: &s[2..] },
        s if s.starts_with('?') => Token::Required { name, check_empty: false, message: &s[1..] },
        _ => Token::InvalidExpr { content },
    }
}
