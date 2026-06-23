//! `Analyzer` — the public analysis entry point for a single Compose file.

use std::path::PathBuf;

use susun_diagnostics::DiagnosticReport;
use susun_loader::{LoadResult, ProjectLoader};
use susun_model::Project;
use susun_normalize::{
    input::MergeProject,
    normalize::{normalize, FinalProjectMetadata},
};
use susun_source::SourceMap;

use crate::Error;

/// Analyzes a single Compose file end-to-end.
///
/// Create one via [`Analyzer::new`], then call [`analyze`][Analyzer::analyze].
/// User-level issues (unknown fields, bad values) appear as diagnostics in
/// [`AnalysisResult::report`] rather than as `Err` returns.
pub struct Analyzer {
    path: PathBuf,
}

/// The result of a successful analysis pipeline run.
pub struct AnalysisResult {
    /// Canonical project, or `None` if the YAML was unrecoverable.
    pub project: Option<Project>,
    /// Source map for all files loaded during this analysis.
    pub source_map: SourceMap,
    /// Diagnostics accumulated from all pipeline stages.
    pub report: DiagnosticReport,
}

impl Analyzer {
    /// Creates an analyzer targeting the given Compose file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Run the full analysis pipeline.
    ///
    /// Returns `Err` only for system-level failures (file not found, I/O
    /// errors). User mistakes produce `Ok` with populated `report.errors`.
    pub fn analyze(self) -> Result<AnalysisResult, Error> {
        let LoadResult { source_map, report: load_report, context, parsed, .. } =
            ProjectLoader::new(&self.path).load()?;
        let mut report = load_report;

        let project = match parsed {
            None => None,
            Some(parsed) => {
                let project_name = context.resolve_project_name(
                    parsed.name.as_ref().map(|s| s.value.as_str()),
                );
                let merge = MergeProject::from(parsed);
                let outcome = normalize(merge, FinalProjectMetadata { project_name })?;
                report.merge(outcome.report);
                Some(outcome.project)
            }
        };

        Ok(AnalysisResult { project, source_map, report })
    }
}
