//! `Analyzer` — the public analysis entry point for a single Compose file.

use std::path::{Path, PathBuf};

use susun_diagnostics::DiagnosticReport;
use susun_loader::ProjectLoader;
use susun_model::{Project, ProjectName};
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
        let load_result = ProjectLoader::new(&self.path).load()?;
        let mut report = load_result.report;

        let project = match load_result.parsed {
            None => None,
            Some(parsed) => {
                let project_name = derive_project_name(
                    parsed.name.as_ref().map(|s| s.value.as_str()),
                    &self.path,
                );
                let merge = MergeProject::from(parsed);
                let outcome = normalize(merge, FinalProjectMetadata { project_name })?;
                report.merge(outcome.report);
                Some(outcome.project)
            }
        };

        Ok(AnalysisResult { project, source_map: load_result.source_map, report })
    }
}

fn derive_project_name(name_from_file: Option<&str>, path: &Path) -> ProjectName {
    if let Some(name) = name_from_file {
        return ProjectName::new(name);
    }
    let dir_name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    ProjectName::new(dir_name)
}
