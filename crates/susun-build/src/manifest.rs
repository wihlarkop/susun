//! Deterministic build input manifests.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::dockerignore::Dockerignore;

/// Deterministic build input manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildInputManifest {
    /// Included files sorted by normalized relative path.
    pub entries: Vec<ManifestEntry>,
}

/// One included build context file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Slash-normalized path relative to the context root.
    pub path: String,
    /// File length in bytes.
    pub len: u64,
    /// Lowercase hex SHA-256 of the file contents.
    pub sha256: String,
}

/// Manifest generation failure.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// Directory traversal failed.
    #[error("failed to read directory `{path}`: {source}")]
    ReadDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// File metadata could not be read.
    #[error("failed to read metadata for `{path}`: {source}")]
    Metadata {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// File contents could not be read.
    #[error("failed to read file `{path}`: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Context entry could not be represented as UTF-8.
    #[error("context path `{path}` is not valid utf-8")]
    NonUtf8Path {
        /// File path.
        path: PathBuf,
    },
}

impl BuildInputManifest {
    /// Enumerates `context_dir`, applies ignore rules, and hashes included files.
    pub fn from_context(
        context_dir: &Path,
        dockerignore: &Dockerignore,
    ) -> Result<Self, ManifestError> {
        let mut entries = Vec::new();
        collect_entries(context_dir, context_dir, dockerignore, &mut entries)?;
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(Self { entries })
    }
}

fn collect_entries(
    root: &Path,
    dir: &Path,
    dockerignore: &Dockerignore,
    entries: &mut Vec<ManifestEntry>,
) -> Result<(), ManifestError> {
    let mut children = fs::read_dir(dir)
        .map_err(|source| ManifestError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ManifestError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;

    children.sort_by_key(|entry| entry.path());

    for child in children {
        let path = child.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| ManifestError::NonUtf8Path { path: path.clone() })?;
        let metadata = fs::symlink_metadata(&path).map_err(|source| ManifestError::Metadata {
            path: path.clone(),
            source,
        })?;
        let is_dir = metadata.is_dir();
        if dockerignore.is_ignored(relative, is_dir) {
            continue;
        }
        if is_dir {
            collect_entries(root, &path, dockerignore, entries)?;
        } else if metadata.is_file() {
            entries.push(hash_file(relative, &path, metadata.len())?);
        }
    }

    Ok(())
}

fn hash_file(relative: &Path, path: &Path, len: u64) -> Result<ManifestEntry, ManifestError> {
    let mut file = fs::File::open(path).map_err(|source| ManifestError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| ManifestError::ReadFile {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(ManifestEntry {
        path: normalize_relative_path(relative)?,
        len,
        sha256: encode_hex(&hasher.finalize()),
    })
}

fn normalize_relative_path(path: &Path) -> Result<String, ManifestError> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Some(segment) = component.as_os_str().to_str() else {
            return Err(ManifestError::NonUtf8Path {
                path: path.to_path_buf(),
            });
        };
        if !segment.is_empty() {
            segments.push(segment);
        }
    }
    Ok(segments.join("/"))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
