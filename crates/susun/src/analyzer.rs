//! `Analyzer` — the public analysis entry point for a single Compose file.

use std::{collections::BTreeMap, path::PathBuf};

use susun_diagnostics::DiagnosticReport;
use susun_loader::{
    LoadContext, MapEnvironment, ProjectLoader, SingleFileResult, load_dotenv_from_path,
};
use susun_model::Project;
use susun_normalize::{
    expand_project,
    merge::merge_projects,
    normalize::{normalize, FinalProjectMetadata},
};
use susun_source::SourceMap;

use crate::Error;

/// Analyzes one or more Compose files end-to-end.
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

        // 3. Inject parsed entry lists and snapshot the process env for reuse
        //    across additional files (env vars must be consistent for all files).
        let context = self
            .context
            .with_dotenv_entries(dotenv_entries.clone())
            .with_env_file_entries(env_file_entries.clone());

        let process_snapshot: BTreeMap<String, String> =
            context.env_vars().into_iter().collect();

        // 4. Build the ordered file list: primary first, then additional.
        let additional_files = context.additional_files.clone();
        let all_files: Vec<PathBuf> = std::iter::once(context.path.clone())
            .chain(additional_files)
            .collect();

        // 5. Load all files into one shared SourceMap so span SourceIds are
        //    globally unique across files. Expand each, then fold with merge.
        let mut source_map = SourceMap::new();
        let mut merged = None;

        for path in &all_files {
            let file_context = LoadContext::new(path)
                .with_env_provider(MapEnvironment::new(process_snapshot.clone()))
                .with_dotenv_entries(dotenv_entries.clone())
                .with_env_file_entries(env_file_entries.clone());

            let SingleFileResult { report: file_report, parsed, .. } =
                ProjectLoader::with_context(file_context).load_into(&mut source_map)?;
            report.merge(file_report);

            if let Some(parsed) = parsed {
                let expanded = expand_project(parsed);
                merged = Some(match merged {
                    None => expanded,
                    Some(base) => merge_projects(base, expanded),
                });
            }
        }

        // 6. Resolve final project name and normalize. The merged name field
        //    already reflects last-overlay-wins from merge_projects.
        let project = match merged {
            None => None,
            Some(merge) => {
                let name_from_file = merge.name.as_ref().map(|s| s.value.as_str());
                let project_name = context.resolve_project_name(name_from_file);
                let outcome = normalize(merge, FinalProjectMetadata { project_name })?;
                report.merge(outcome.report);
                Some(outcome.project)
            }
        };

        Ok(AnalysisResult { project, source_map, report })
    }
}
