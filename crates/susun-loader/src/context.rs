//! Load context: configuration for a single loader invocation.

use std::path::{Path, PathBuf};

use susun_model::ProjectName;

use std::collections::BTreeMap;

use crate::environment::{EnvResolver, EnvironmentProvider, MapEnvironment, ProcessEnvironment};

/// Configuration for a single [`ProjectLoader`][crate::loader::ProjectLoader] run.
///
/// Build with [`LoadContext::new`] and chain builder methods to override
/// defaults before passing to `ProjectLoader`.
pub struct LoadContext {
    /// Path to the primary Compose file.
    pub path: PathBuf,
    /// Explicit project name supplied by the caller (e.g. `--project-name`).
    ///
    /// When set, overrides all other project name sources.
    pub project_name_override: Option<String>,
    /// Environment variable provider.
    ///
    /// Defaults to [`ProcessEnvironment`]. Replace with [`MapEnvironment`][crate::environment::MapEnvironment]
    /// for deterministic tests.
    env_provider: Box<dyn EnvironmentProvider>,
}

impl LoadContext {
    /// Creates a context for the given path with default settings.
    ///
    /// Defaults: no project name override, process environment.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            project_name_override: None,
            env_provider: Box::new(ProcessEnvironment),
        }
    }

    /// Override the project name (equivalent to `--project-name`).
    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name_override = Some(name.into());
        self
    }

    /// Replace the environment provider (use [`MapEnvironment`][crate::environment::MapEnvironment] in tests).
    pub fn with_env_provider(mut self, provider: impl EnvironmentProvider + 'static) -> Self {
        self.env_provider = Box::new(provider);
        self
    }

    /// Resolves the canonical project name using the four-level precedence:
    ///
    /// 1. Explicit override (`--project-name` / [`LoadContext::with_project_name`])
    /// 2. `COMPOSE_PROJECT_NAME` environment variable (non-empty)
    /// 3. `name:` field from the parsed Compose file
    /// 4. Parent directory name of the Compose file path
    pub fn resolve_project_name(&self, name_from_file: Option<&str>) -> ProjectName {
        if let Some(name) = &self.project_name_override {
            return ProjectName::new(name);
        }
        if let Some(name) = self.env_provider.get("COMPOSE_PROJECT_NAME") {
            if !name.is_empty() {
                return ProjectName::new(&name);
            }
        }
        if let Some(name) = name_from_file {
            return ProjectName::new(name);
        }
        ProjectName::new(fallback_dir_name(&self.path))
    }

    /// Reads a variable from the configured environment provider.
    pub fn env_get(&self, key: &str) -> Option<String> {
        self.env_provider.get(key)
    }

    /// Builds an [`EnvResolver`] from the current environment stack.
    ///
    /// Snapshots the environment provider's variables into a [`MapEnvironment`]
    /// so the resolver can be passed by value to the parser. In Phase 1 the
    /// `--env-file` and default `.env` layers are empty (wired in Task 17).
    pub fn build_resolver(&self) -> EnvResolver {
        let map: BTreeMap<String, String> = self.env_provider.vars().into_iter().collect();
        EnvResolver::new(MapEnvironment::new(map), vec![], vec![])
    }
}

fn fallback_dir_name(path: &Path) -> &str {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("project")
}
