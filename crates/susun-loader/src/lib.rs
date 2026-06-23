//! Loads Compose files from sources into parsed form ready for normalization.
//!
//! All `saphyr` YAML types are confined inside this crate and never appear
//! in public signatures.

pub mod environment;
pub mod error;
pub mod loader;
pub(crate) mod parser;
pub mod context;

pub use context::LoadContext;
pub use environment::{EnvironmentProvider, MapEnvironment, ProcessEnvironment};
pub use error::LoadError;
pub use loader::{LoadResult, ProjectLoader};

use susun_diagnostics::DiagnosticReport;
use susun_normalize::input::ParsedProject;
use susun_source::SourceId;

/// Parse a Compose YAML string into a raw [`ParsedProject`].
///
/// User errors (malformed YAML, unknown fields, multiple documents) are
/// appended to `report` as diagnostics. Returns `None` only when the YAML
/// is unrecoverable; recoverable issues still yield `Some`.
pub fn parse_compose_str(
    source_id: SourceId,
    contents: &str,
    report: &mut DiagnosticReport,
) -> Option<ParsedProject> {
    parser::parse(source_id, contents, report)
}
