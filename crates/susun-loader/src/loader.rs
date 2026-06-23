//! `ProjectLoader` — reads and parses a single Compose file.

use std::{path::PathBuf, sync::Arc};

use susun_diagnostics::DiagnosticReport;
use susun_normalize::input::ParsedProject;
use susun_source::{FileSystemSourceProvider, SourceId, SourceMap, SourceProvider, SourceRequest};

use crate::{context::LoadContext, error::LoadError, parser};

/// Raw result of loading a single Compose file into a caller-supplied source map.
///
/// Obtained from [`ProjectLoader::load_into`]. The source was registered into
/// the caller's [`SourceMap`] before parsing, so all spans use IDs from that map.
pub struct SingleFileResult {
    /// Diagnostics collected during parsing.
    pub report: DiagnosticReport,
    /// Source id of the file in the caller's source map.
    pub source_id: SourceId,
    /// The raw parsed project, or `None` if YAML was unrecoverable.
    pub parsed: Option<ParsedProject>,
    /// The load context used for this run.
    pub context: LoadContext,
}

/// Raw result of loading and parsing a single Compose file with its own source map.
///
/// User-level issues (malformed YAML, unknown fields) are in `report`.
/// `parsed` is `None` only when the YAML cannot be recovered at all.
pub struct LoadResult {
    /// Sources registered during this load (one per file for Phase 1).
    pub source_map: SourceMap,
    /// Diagnostics collected during parsing.
    pub report: DiagnosticReport,
    /// The source id of the primary file, for cross-referencing with `source_map`.
    pub source_id: SourceId,
    /// The raw parsed project, or `None` if YAML was unrecoverable.
    pub parsed: Option<ParsedProject>,
    /// The load context used for this run, returned for project-name resolution.
    pub context: LoadContext,
}

/// Loads a single Compose file from the filesystem.
///
/// Inject a custom [`SourceProvider`] with [`ProjectLoader::with_provider`]
/// for testing or sandboxed execution.
pub struct ProjectLoader {
    context: LoadContext,
    provider: Box<dyn SourceProvider>,
}

impl ProjectLoader {
    /// Creates a loader for the given path with default settings.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            context: LoadContext::new(path),
            provider: Box::new(FileSystemSourceProvider::with_default_limits()),
        }
    }

    /// Creates a loader with a fully configured [`LoadContext`].
    pub fn with_context(context: LoadContext) -> Self {
        Self {
            context,
            provider: Box::new(FileSystemSourceProvider::with_default_limits()),
        }
    }

    /// Creates a loader with a custom [`SourceProvider`] (primarily for tests).
    pub fn with_provider(path: impl Into<PathBuf>, provider: impl SourceProvider + 'static) -> Self {
        Self {
            context: LoadContext::new(path),
            provider: Box::new(provider),
        }
    }

    /// Creates a loader with both a custom context and a custom source provider.
    pub fn with_context_and_provider(
        context: LoadContext,
        provider: impl SourceProvider + 'static,
    ) -> Self {
        Self { context, provider: Box::new(provider) }
    }

    /// Read, parse, and register the source into `source_map`.
    ///
    /// Returns `Err` only for system-level failures (file not found, I/O error).
    /// User-level mistakes yield `Ok` with diagnostics in the result's `report`.
    /// All span `SourceId` values reference entries in the provided `source_map`.
    pub fn load_into(
        self,
        source_map: &mut SourceMap,
    ) -> Result<SingleFileResult, LoadError> {
        let path = self.context.path.clone();
        let request = SourceRequest::new(&path);
        let loaded = self
            .provider
            .read(&request)
            .map_err(|e| LoadError::from_provider(path.clone(), e))?;

        let source_id = source_map.register(loaded);

        let contents: Arc<str> = source_map
            .get(source_id)
            .map(|s| Arc::clone(&s.contents))
            .ok_or_else(|| LoadError::Read {
                path: path.clone(),
                message: "source disappeared after registration".to_owned(),
            })?;

        let mut report = DiagnosticReport::new();
        let resolver = self.context.build_resolver();
        let parsed = parser::parse(source_id, contents.as_ref(), &resolver, &mut report);

        Ok(SingleFileResult { report, source_id, parsed, context: self.context })
    }

    /// Read, parse, and return the raw load result with a fresh source map.
    ///
    /// Convenience wrapper around [`load_into`][Self::load_into].
    pub fn load(self) -> Result<LoadResult, LoadError> {
        let mut source_map = SourceMap::new();
        let SingleFileResult { report, source_id, parsed, context } =
            self.load_into(&mut source_map)?;
        Ok(LoadResult { source_map, report, source_id, parsed, context })
    }
}

