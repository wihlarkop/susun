//! Build context and Dockerfile path resolution.

use std::path::{Path, PathBuf};

use susun_model::BuildDefinition;
use thiserror::Error;

/// Resolved build input paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInputPaths {
    /// Canonical project directory.
    pub project_dir: PathBuf,
    /// Canonical build context directory.
    pub context_dir: PathBuf,
    /// Canonical Dockerfile path.
    pub dockerfile: PathBuf,
}

/// Build input resolution failure.
#[derive(Debug, Error)]
pub enum BuildResolveError {
    /// Project directory could not be resolved.
    #[error("failed to resolve project directory `{path}`: {source}")]
    ProjectDirectory {
        /// Path that failed.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Build context could not be resolved.
    #[error("failed to resolve build context `{path}`: {source}")]
    Context {
        /// Path that failed.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Dockerfile could not be resolved.
    #[error("failed to resolve dockerfile `{path}`: {source}")]
    Dockerfile {
        /// Path that failed.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Resolved context escaped the project directory.
    #[error("build context `{context}` escapes project directory `{project}`")]
    ContextEscapesProject {
        /// Canonical project directory.
        project: PathBuf,
        /// Canonical context directory.
        context: PathBuf,
    },
    /// Resolved Dockerfile escaped the build context.
    #[error("dockerfile `{dockerfile}` escapes build context `{context}`")]
    DockerfileEscapesContext {
        /// Canonical build context directory.
        context: PathBuf,
        /// Canonical Dockerfile path.
        dockerfile: PathBuf,
    },
}

/// Resolves a service build definition into canonical local input paths.
pub fn resolve_build_inputs(
    project_dir: &Path,
    build: &BuildDefinition,
) -> Result<BuildInputPaths, BuildResolveError> {
    let project_dir =
        project_dir
            .canonicalize()
            .map_err(|source| BuildResolveError::ProjectDirectory {
                path: project_dir.to_path_buf(),
                source,
            })?;

    let context_value = build.context.as_deref().unwrap_or(".");
    let context_path = join_project_relative(&project_dir, context_value);
    let context_dir = context_path
        .canonicalize()
        .map_err(|source| BuildResolveError::Context {
            path: context_path.clone(),
            source,
        })?;

    if !context_dir.starts_with(&project_dir) {
        return Err(BuildResolveError::ContextEscapesProject {
            project: project_dir,
            context: context_dir,
        });
    }

    let dockerfile_value = build.dockerfile.as_deref().unwrap_or("Dockerfile");
    let dockerfile_path = if Path::new(dockerfile_value).is_absolute() {
        PathBuf::from(dockerfile_value)
    } else {
        context_dir.join(dockerfile_value)
    };
    let dockerfile =
        dockerfile_path
            .canonicalize()
            .map_err(|source| BuildResolveError::Dockerfile {
                path: dockerfile_path.clone(),
                source,
            })?;

    if !dockerfile.starts_with(&context_dir) {
        return Err(BuildResolveError::DockerfileEscapesContext {
            context: context_dir,
            dockerfile,
        });
    }

    Ok(BuildInputPaths {
        project_dir,
        context_dir,
        dockerfile,
    })
}

fn join_project_relative(project_dir: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    }
}
