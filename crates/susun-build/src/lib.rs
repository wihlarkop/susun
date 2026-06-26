//! Neutral build support for Susun.
//!
//! This crate prepares Compose build inputs without depending on Docker,
//! BuildKit, Bollard, or registry implementation details.

pub mod dockerignore;
pub mod manifest;
pub mod resolve;

pub use dockerignore::{Dockerignore, DockerignorePattern};
pub use manifest::{BuildInputManifest, ManifestEntry, ManifestError};
pub use resolve::{BuildInputPaths, BuildResolveError, resolve_build_inputs};
