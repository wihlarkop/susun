//! Loads Compose files from sources into parsed form ready for normalization.
//!
//! All `saphyr` YAML types are confined inside this crate and never appear
//! in public signatures.

pub mod environment;
pub mod error;
pub mod interpolation;
pub mod loader;
pub(crate) mod parser;
pub mod context;

pub use context::LoadContext;
pub use environment::{DotenvEntry, EnvResolver, EnvironmentProvider, MapEnvironment, ProcessEnvironment, parse_dotenv};
pub use error::LoadError;
pub use loader::{LoadResult, ProjectLoader};

use std::{path::PathBuf, sync::Arc};

use susun_diagnostics::DiagnosticReport;
use susun_normalize::input::ParsedProject;
use susun_source::{FileSystemSourceProvider, SourceId, SourceMap, SourceProvider, SourceRequest};

/// Reads and parses a `.env` file from the filesystem, returning its entries.
///
/// If the file does not exist, [`LoadError::NotFound`] is returned. Other I/O
/// failures return [`LoadError::Read`] or [`LoadError::NotUtf8`].
///
/// Diagnostic codes `SUS-ENV-002` and `SUS-ENV-003` from the parse step are
/// added to `report` as usual.
pub fn load_dotenv_from_path(
    path: impl Into<PathBuf>,
    report: &mut DiagnosticReport,
) -> Result<Vec<DotenvEntry>, LoadError> {
    let path = path.into();
    let provider = FileSystemSourceProvider::with_default_limits();
    let request = SourceRequest::new(&path);
    let loaded = provider
        .read(&request)
        .map_err(|e| LoadError::from_provider(path.clone(), e))?;

    let mut sm = SourceMap::new();
    let contents: Arc<str> = Arc::clone(&loaded.contents);
    let source_id = sm.register(loaded);

    Ok(environment::dotenv::parse_dotenv(source_id, &contents, report))
}

/// Parse a Compose YAML string into a raw [`ParsedProject`].
///
/// User errors (malformed YAML, unknown fields, multiple documents) are
/// appended to `report` as diagnostics. Returns `None` only when the YAML
/// is unrecoverable; recoverable issues still yield `Some`.
///
/// Scalar values are interpolated with an empty resolver (no environment
/// variables). Use [`ProjectLoader`] when environment-variable substitution
/// is required.
pub fn parse_compose_str(
    source_id: SourceId,
    contents: &str,
    report: &mut DiagnosticReport,
) -> Option<ParsedProject> {
    let resolver = EnvResolver::new(environment::MapEnvironment::default(), vec![], vec![]);
    parser::parse(source_id, contents, &resolver, report)
}
