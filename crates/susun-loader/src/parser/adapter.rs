//! Converts saphyr YAML nodes to source-span-aware parsed types.
//!
//! All saphyr types are confined to this module. Nothing from `saphyr` or
//! `saphyr_parser` appears in public signatures outside of `susun-loader`.

use indexmap::IndexMap;
use saphyr::{LoadableYamlNode, MarkedYamlOwned, YamlDataOwned};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Label, Severity};
use susun_normalize::input::{ParsedProject, ParsedService};
use susun_source::{SourceId, Span, Spanned, TextOffset};

use crate::{environment::resolve::EnvResolver, interpolation::eval::interpolate};

const PARSE_ERROR: &str = "SUS-PARSE-001";
const MULTI_DOC: &str = "SUS-PARSE-003";
const ROOT_NOT_MAPPING: &str = "SUS-PARSE-010";
const UNKNOWN_FIELD: &str = "SUS-PARSE-011";

/// Parse `contents` (a single Compose YAML file) into a raw [`ParsedProject`].
///
/// Scalar values are interpolated using `resolver` before typed extraction.
/// Mapping keys are never interpolated. User errors are appended to `report`
/// as diagnostics. Returns `None` only when YAML is so malformed that no
/// structure can be recovered; recoverable problems still yield `Some`.
pub(crate) fn parse(
    source_id: SourceId,
    contents: &str,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<ParsedProject> {
    let docs = match MarkedYamlOwned::load_from_str(contents) {
        Ok(docs) => docs,
        Err(e) => {
            report.push(Diagnostic::new(PARSE_ERROR, Severity::Error, e.to_string()));
            return None;
        }
    };

    if docs.is_empty() {
        return Some(ParsedProject { name: None, services: IndexMap::new() });
    }

    if docs.len() > 1 {
        let second = &docs[1];
        let span = node_span(contents, source_id, second);
        report.push(
            Diagnostic::new(
                MULTI_DOC,
                Severity::Error,
                "Compose files must contain exactly one YAML document",
            )
            .with_label(Label::primary(span, "second document starts here")),
        );
        return None;
    }

    let Some(doc) = docs.into_iter().next() else {
        return Some(ParsedProject { name: None, services: IndexMap::new() });
    };

    let mapping = match doc.data {
        YamlDataOwned::Mapping(m) => m,
        _ => {
            let span = node_span(contents, source_id, &doc);
            report.push(
                Diagnostic::new(
                    ROOT_NOT_MAPPING,
                    Severity::Error,
                    "Compose file root must be a YAML mapping",
                )
                .with_label(Label::primary(span, "expected mapping here")),
            );
            return None;
        }
    };

    let mut project_name: Option<Spanned<String>> = None;
    let mut services: IndexMap<String, Spanned<ParsedService>> = IndexMap::new();

    for (k_node, v_node) in &mapping {
        // Keys are never interpolated.
        let Some(key) = node_as_str(k_node) else {
            continue;
        };

        match key {
            "name" => {
                if let Some(interpolated) =
                    interpolated_scalar(contents, source_id, v_node, resolver, report)
                {
                    let span = node_span(contents, source_id, v_node);
                    project_name = Some(Spanned::new(interpolated, span));
                }
            }
            "services" => {
                services = parse_services(source_id, contents, v_node, resolver, report);
            }
            // Accepted top-level fields not yet extracted (normalizer handles them later)
            "networks" | "volumes" | "configs" | "secrets" | "profiles" => {}
            // Extension fields accepted silently
            k if k.starts_with("x-") => {}
            // Deferred fields: known but not yet supported
            "version" | "deploy" | "include" | "extends" | "develop" | "watch" => {
                let span = node_span(contents, source_id, k_node);
                report.push(
                    Diagnostic::new(
                        UNKNOWN_FIELD,
                        Severity::Note,
                        format!("field `{key}` is not yet supported and will be ignored"),
                    )
                    .with_label(Label::primary(span, "deferred field")),
                );
            }
            // Truly unknown fields
            _ => {
                let span = node_span(contents, source_id, k_node);
                report.push(
                    Diagnostic::new(
                        UNKNOWN_FIELD,
                        Severity::Warning,
                        format!("unknown top-level field `{key}`"),
                    )
                    .with_label(Label::primary(span, "unknown field")),
                );
            }
        }
    }

    Some(ParsedProject { name: project_name, services })
}

fn parse_services(
    source_id: SourceId,
    contents: &str,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> IndexMap<String, Spanned<ParsedService>> {
    let mapping = match &node.data {
        YamlDataOwned::Mapping(m) => m,
        _ => return IndexMap::new(),
    };

    let mut result: IndexMap<String, Spanned<ParsedService>> = IndexMap::new();
    for (k_node, v_node) in mapping {
        // Service names (keys) are not interpolated.
        let Some(name) = node_as_str(k_node) else {
            continue;
        };
        let service = parse_service(source_id, contents, v_node, resolver, report);
        let span = node_span(contents, source_id, v_node);
        result.insert(name.to_owned(), Spanned::new(service, span));
    }
    result
}

fn parse_service(
    source_id: SourceId,
    contents: &str,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    _report: &mut DiagnosticReport,
) -> ParsedService {
    let mapping = match &node.data {
        YamlDataOwned::Mapping(m) => m,
        _ => return ParsedService { image: None },
    };

    let mut image: Option<Spanned<String>> = None;
    for (k_node, v_node) in mapping {
        // Field names (keys) are not interpolated.
        let Some(key) = node_as_str(k_node) else {
            continue;
        };
        if key == "image" {
            if let Some(interpolated) =
                interpolated_scalar(contents, source_id, v_node, resolver, _report)
            {
                let span = node_span(contents, source_id, v_node);
                image = Some(Spanned::new(interpolated, span));
            }
        }
    }

    ParsedService { image }
}

/// Extracts a scalar string from `node` and applies environment interpolation.
///
/// Returns `None` if the node is not a scalar. Any `SUS-ENV-001` diagnostics
/// from required-variable failures are appended to `report`.
fn interpolated_scalar(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<String> {
    let raw = node_as_str(node)?;
    let span = node_span(contents, source_id, node);
    Some(interpolate(raw, resolver, span, report))
}

/// Extract a string value from a YAML node, handling both `Value` and tagged `Representation`.
fn node_as_str(node: &MarkedYamlOwned) -> Option<&str> {
    match &node.data {
        YamlDataOwned::Value(v) => v.as_str(),
        YamlDataOwned::Representation(v, _, _) => Some(v.as_str()),
        _ => None,
    }
}

/// Convert a saphyr node's char-based span into a byte-offset [`Span`].
fn node_span(contents: &str, source_id: SourceId, node: &MarkedYamlOwned) -> Span {
    let start_byte = char_to_byte(contents, node.span.start.index()) as u32;
    let end_byte = char_to_byte(contents, node.span.end.index()) as u32;
    let start = TextOffset::new(start_byte);
    // Guard against end < start (defensive; saphyr should never produce this)
    let end = TextOffset::new(end_byte.max(start_byte));
    Span::new(source_id, start, end)
        .unwrap_or_else(|_| Span::empty(source_id, start))
}

/// Convert a saphyr char-index to a UTF-8 byte offset in `src`.
fn char_to_byte(src: &str, char_index: usize) -> usize {
    src.char_indices()
        .nth(char_index)
        .map_or(src.len(), |(b, _)| b)
}
