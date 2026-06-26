//! `Analyzer` — the public analysis entry point for a single Compose file.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_graph::DependencyGraph;
use susun_loader::{
    LoadContext, MapEnvironment, ProjectLoader, SingleFileResult, load_dotenv_from_path,
};
use susun_model::Project;
use susun_normalize::{
    expand_project,
    input::MergeProject,
    merge::{merge_projects, merge_services},
    normalize::{FinalProjectMetadata, normalize},
    selection::{ProjectSelection, select_services},
};
use susun_source::SourceMap;

use crate::Error;

const INCLUDE_CYCLE: &str = "SUS-INC-001";
const INCLUDE_DEPTH: &str = "SUS-INC-002";
const INCLUDE_LIMIT: &str = "SUS-INC-003";
const EXTENDS_MISSING: &str = "SUS-EXT-001";
const EXTENDS_CYCLE: &str = "SUS-EXT-002";
const INCLUDE_MAX_DEPTH: usize = 16;
const INCLUDE_MAX_FILES: usize = 128;

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
    /// Active service selection, or `None` if no project was produced.
    pub selection: Option<ProjectSelection>,
    /// Dependency graph, or `None` if no project was produced or a cycle exists.
    pub graph: Option<DependencyGraph>,
    /// Source map for all files loaded during this analysis.
    pub source_map: SourceMap,
    /// Diagnostics accumulated from all pipeline stages.
    pub report: DiagnosticReport,
}

impl Analyzer {
    /// Creates an analyzer targeting the given Compose file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            context: LoadContext::new(path),
            env_file: None,
        }
    }

    /// Creates an analyzer with a fully configured [`LoadContext`].
    ///
    /// Use this when you need to inject an env provider or set a project name
    /// override without going through the default CLI path.
    pub fn with_context(context: LoadContext) -> Self {
        Self {
            context,
            env_file: None,
        }
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

        let process_snapshot: BTreeMap<String, String> = context.env_vars().into_iter().collect();

        // 4. Build the ordered file list: primary first, then additional.
        let additional_files = context.additional_files.clone();
        let all_files: Vec<PathBuf> = std::iter::once(context.path.clone())
            .chain(additional_files)
            .collect();

        // 5. Load all files into one shared SourceMap so span SourceIds are
        //    globally unique across files. Expand each, then fold with merge.
        let mut source_map = SourceMap::new();
        let mut merged = None;
        let mut include_loader = IncludeLoader::new(
            process_snapshot,
            env_file_entries.clone(),
            &mut source_map,
            &mut report,
        );

        for path in &all_files {
            let mut parsed_files = Vec::new();
            include_loader.load(path, &mut parsed_files, &mut Vec::new(), 0)?;

            for parsed in parsed_files {
                let expanded = expand_project(parsed);
                merged = Some(match merged {
                    None => expanded,
                    Some(base) => merge_projects(base, expanded),
                });
            }
        }

        // 6. Resolve final project name and normalize. The merged name field
        //    already reflects last-overlay-wins from merge_projects.
        let project_directory = context
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut selection = None;
        let mut graph = None;

        let project = match merged {
            None => None,
            Some(mut merge) => {
                resolve_extends(&mut merge, &mut report);
                let name_from_file = merge.name.as_ref().map(|s| s.value.as_str());
                let project_name = context.resolve_project_name(name_from_file);
                let outcome = normalize(
                    merge,
                    FinalProjectMetadata {
                        project_name,
                        project_directory,
                    },
                )?;
                report.merge(outcome.report);
                let selected = select_services(&outcome.project, &context.profiles);
                let validation = susun_validation::validate(&outcome.project, &selected);
                report.merge(validation.report);
                let graph_outcome = susun_graph::build_graph(&outcome.project, &selected);
                report.merge(graph_outcome.report);
                graph = graph_outcome.graph;
                selection = Some(selected);
                Some(outcome.project)
            }
        };

        Ok(AnalysisResult {
            project,
            selection,
            graph,
            source_map,
            report,
        })
    }
}

struct IncludeLoader<'a> {
    process_snapshot: BTreeMap<String, String>,
    env_file_entries: Vec<susun_loader::DotenvEntry>,
    source_map: &'a mut SourceMap,
    report: &'a mut DiagnosticReport,
    loaded_count: usize,
}

impl<'a> IncludeLoader<'a> {
    fn new(
        process_snapshot: BTreeMap<String, String>,
        env_file_entries: Vec<susun_loader::DotenvEntry>,
        source_map: &'a mut SourceMap,
        report: &'a mut DiagnosticReport,
    ) -> Self {
        Self {
            process_snapshot,
            env_file_entries,
            source_map,
            report,
            loaded_count: 0,
        }
    }

    fn load(
        &mut self,
        path: &Path,
        parsed_files: &mut Vec<susun_normalize::input::ParsedProject>,
        stack: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > INCLUDE_MAX_DEPTH {
            self.report.push(Diagnostic::new(
                INCLUDE_DEPTH,
                Severity::Error,
                format!("include depth exceeds limit of {INCLUDE_MAX_DEPTH}"),
            ));
            return Ok(());
        }
        if self.loaded_count >= INCLUDE_MAX_FILES {
            self.report.push(Diagnostic::new(
                INCLUDE_LIMIT,
                Severity::Error,
                format!("include file count exceeds limit of {INCLUDE_MAX_FILES}"),
            ));
            return Ok(());
        }

        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if stack.contains(&key) {
            self.report.push(Diagnostic::new(
                INCLUDE_CYCLE,
                Severity::Error,
                format!("include cycle detected at `{}`", path.display()),
            ));
            return Ok(());
        }

        let local_dotenv = path
            .parent()
            .map(|dir| dir.join(".env"))
            .map(|dotenv| load_dotenv_from_path(&dotenv, self.report).unwrap_or_default())
            .unwrap_or_default();
        let file_context = LoadContext::new(path)
            .with_env_provider(MapEnvironment::new(self.process_snapshot.clone()))
            .with_dotenv_entries(local_dotenv)
            .with_env_file_entries(self.env_file_entries.clone());

        let SingleFileResult {
            report: file_report,
            parsed,
            ..
        } = ProjectLoader::with_context(file_context).load_into(self.source_map)?;
        self.loaded_count += 1;
        self.report.merge(file_report);

        let Some(parsed) = parsed else {
            return Ok(());
        };

        stack.push(key);
        for include in &parsed.includes {
            let include_path = resolve_related_path(path, &include.value);
            self.load(&include_path, parsed_files, stack, depth + 1)?;
        }
        stack.pop();

        parsed_files.push(parsed);
        Ok(())
    }
}

fn resolve_related_path(base_file: &Path, related: &str) -> PathBuf {
    let path = PathBuf::from(related);
    if path.is_absolute() {
        path
    } else {
        base_file
            .parent()
            .map(|parent| parent.join(&path))
            .unwrap_or(path)
    }
}

fn resolve_extends(project: &mut MergeProject, report: &mut DiagnosticReport) {
    let names: Vec<String> = project.services.keys().cloned().collect();
    let mut resolved = BTreeMap::new();
    for name in names {
        let _ = resolve_extends_for(&name, project, report, &mut Vec::new(), &mut resolved);
    }
}

fn resolve_extends_for(
    name: &str,
    project: &mut MergeProject,
    report: &mut DiagnosticReport,
    stack: &mut Vec<String>,
    resolved: &mut BTreeMap<String, bool>,
) -> bool {
    if resolved.get(name).copied().unwrap_or(false) {
        return true;
    }
    if stack.iter().any(|item| item == name) {
        report.push(Diagnostic::new(
            EXTENDS_CYCLE,
            Severity::Error,
            format!("extends cycle detected at service `{name}`"),
        ));
        return false;
    }

    let Some(service) = project.services.get(name).cloned() else {
        report.push(Diagnostic::new(
            EXTENDS_MISSING,
            Severity::Error,
            format!("extended service `{name}` was not found"),
        ));
        return false;
    };
    let Some(extends) = service.value.extends.clone() else {
        resolved.insert(name.to_owned(), true);
        return true;
    };

    let base_name = extends.service.value;
    if !project.services.contains_key(&base_name) {
        report.push(Diagnostic::new(
            EXTENDS_MISSING,
            Severity::Error,
            format!("service `{name}` extends missing service `{base_name}`"),
        ));
        return false;
    }

    stack.push(name.to_owned());
    if !resolve_extends_for(&base_name, project, report, stack, resolved) {
        stack.pop();
        return false;
    }
    stack.pop();

    let Some(base) = project.services.get(&base_name).cloned() else {
        return false;
    };
    let mut overlay = service.value;
    overlay.extends = None;
    let merged = merge_services(base.value, overlay);
    if let Some(slot) = project.services.get_mut(name) {
        slot.value = merged;
    }
    resolved.insert(name.to_owned(), true);
    true
}
