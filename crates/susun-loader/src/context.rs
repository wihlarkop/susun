//! Load context: configuration for a single loader invocation.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use susun_model::ProjectName;

use crate::environment::{
    DotenvEntry, EnvResolver, EnvironmentProvider, MapEnvironment, ProcessEnvironment,
};

/// Configuration for a single [`ProjectLoader`][crate::loader::ProjectLoader] run.
///
/// Build with [`LoadContext::new`] and chain builder methods to override
/// defaults before passing to `ProjectLoader`.
pub struct LoadContext {
    /// Path to the primary Compose file.
    pub path: PathBuf,
    /// Additional Compose files specified via repeated `-f` flags, processed
    /// after the primary file in declaration order.
    pub additional_files: Vec<PathBuf>,
    /// Explicit project name supplied by the caller (e.g. `--project-name`).
    ///
    /// When set, overrides all other project name sources.
    pub project_name_override: Option<String>,
    /// Environment variable provider.
    ///
    /// Defaults to [`ProcessEnvironment`]. Replace with [`MapEnvironment`][crate::environment::MapEnvironment]
    /// for deterministic tests.
    env_provider: Box<dyn EnvironmentProvider>,
    /// Entries from the explicit `--env-file`, if provided.
    env_file_entries: Vec<DotenvEntry>,
    /// Entries from the auto-discovered `.env` file, if present.
    dotenv_entries: Vec<DotenvEntry>,
    /// Active profiles for service filtering (applied in Task 27).
    pub profiles: Vec<String>,
}

impl LoadContext {
    /// Creates a context for the given path with default settings.
    ///
    /// Defaults: no project name override, process environment, no profiles.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            additional_files: Vec::new(),
            project_name_override: None,
            env_provider: Box::new(ProcessEnvironment),
            env_file_entries: Vec::new(),
            dotenv_entries: Vec::new(),
            profiles: Vec::new(),
        }
    }

    /// Set additional Compose files to load and merge after the primary file.
    ///
    /// Files are processed in declaration order; each overlays the previous.
    pub fn with_additional_files(mut self, files: Vec<PathBuf>) -> Self {
        self.additional_files = files;
        self
    }

    /// Override the project name (equivalent to `--project-name`).
    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name_override = Some(name.into());
        self
    }

    /// Replace the environment provider (use [`MapEnvironment`] in tests).
    pub fn with_env_provider(mut self, provider: impl EnvironmentProvider + 'static) -> Self {
        self.env_provider = Box::new(provider);
        self
    }

    /// Set entries from an explicit `--env-file` (highest file precedence).
    pub fn with_env_file_entries(mut self, entries: Vec<DotenvEntry>) -> Self {
        self.env_file_entries = entries;
        self
    }

    /// Set entries from the auto-discovered `.env` file (lowest file precedence).
    pub fn with_dotenv_entries(mut self, entries: Vec<DotenvEntry>) -> Self {
        self.dotenv_entries = entries;
        self
    }

    /// Set the active profile names for service filtering.
    pub fn with_profiles(mut self, profiles: Vec<String>) -> Self {
        self.profiles = profiles;
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

    /// Returns all variables from the configured environment provider.
    ///
    /// Useful for snapshotting the environment when constructing contexts for
    /// additional `-f` files that must share the same environment.
    pub fn env_vars(&self) -> Vec<(String, String)> {
        self.env_provider.vars()
    }

    /// Builds an [`EnvResolver`] from the current environment stack.
    ///
    /// Snapshots the environment provider's variables into a [`MapEnvironment`]
    /// combined with the already-parsed `--env-file` and `.env` entry lists.
    pub fn build_resolver(&self) -> EnvResolver {
        let map: BTreeMap<String, String> = self.env_provider.vars().into_iter().collect();
        EnvResolver::new(
            MapEnvironment::new(map),
            self.env_file_entries.clone(),
            self.dotenv_entries.clone(),
        )
    }
}

fn fallback_dir_name(path: &Path) -> &str {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("project")
}
