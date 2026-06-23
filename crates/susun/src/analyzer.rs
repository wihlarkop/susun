//! `Analyzer` — the public analysis entry point for a single Compose file.

use std::path::PathBuf;

use susun_diagnostics::DiagnosticReport;
use susun_loader::{LoadContext, LoadResult, ProjectLoader, load_dotenv_from_path};
use susun_model::Project;
use susun_normalize::{
    expand_project,
    normalize::{normalize, FinalProjectMetadata},
};
use susun_source::SourceMap;

use crate::Error;

/// Analyzes a single Compose file end-to-end.
///
/// Create one via [`Analyzer::new`], optionally chain builder methods, then
/// call [`analyze`][Analyzer::analyze]. User-level issues (unknown fields,
/// bad values, missing required variables) appear as diagnostics in
/// [`AnalysisResult::report`] rather than as `Err` returns.
pub struct Analyzer {
    context: LoadContext,
    /// Optional explicit `.env`-format file (from `--env-file`).
    env_file: Option<PathBuf>,
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
        Self { context: LoadContext::new(path), env_file: None }
    }

    /// Creates an analyzer with a fully configured [`LoadContext`].
    ///
    /// Use this when you need to inject an env provider or set a project name
    /// override without going through the default CLI path.
    pub fn with_context(context: LoadContext) -> Self {
        Self { context, env_file: None }
    }

    /// Specifies a `.env`-format file to load (equivalent to `--env-file`).
    ///
    /// Its variables take precedence over the auto-discovered `.env` file but
    /// are overridden by the process environment.
    pub fn with_env_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.env_file = Some(path.into());
        self
    }

    /// Run the full analysis pipeline.
    ///
    /// Returns `Err` only for system-level failures (file not found, I/O
    /// errors). User mistakes produce `Ok` with populated `report.errors`.
    pub fn analyze(self) -> Result<AnalysisResult, Error> {
        let mut report = DiagnosticReport::new();

        // 1. Auto-discover the default `.env` from the compose file directory.
        let compose_dir = self.context.path.parent().map(|p| p.to_path_buf());
        let dotenv_entries = if let Some(dir) = compose_dir {
            let dotenv_path = dir.join(".env");
            load_dotenv_from_path(&dotenv_path, &mut report).unwrap_or_default()
        } else {
            Vec::new()
        };

        // 2. Load the explicit `--env-file`, if any (errors here are fatal).
        let env_file_entries = if let Some(path) = &self.env_file {
            load_dotenv_from_path(path, &mut report)?
        } else {
            Vec::new()
        };

        // 3. Inject the parsed entry lists into the context.
        let context = self
            .context
            .with_dotenv_entries(dotenv_entries)
            .with_env_file_entries(env_file_entries);

        let LoadResult { source_map, report: load_report, context, parsed, .. } =
            ProjectLoader::with_context(context).load()?;
        report.merge(load_report);

        let project = match parsed {
            None => None,
            Some(parsed) => {
                let project_name = context.resolve_project_name(
                    parsed.name.as_ref().map(|s| s.value.as_str()),
                );
                let merge = expand_project(parsed);
                let outcome = normalize(merge, FinalProjectMetadata { project_name })?;
                report.merge(outcome.report);
                Some(outcome.project)
            }
        };

        Ok(AnalysisResult { project, source_map, report })
    }
}
