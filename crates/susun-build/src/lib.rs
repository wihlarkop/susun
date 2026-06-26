//! Neutral build support for Susun.
//!
//! This crate prepares Compose build inputs without depending on Docker,
//! BuildKit, Bollard, or registry implementation details.

pub mod buildx;
pub mod dockerfile;
pub mod dockerignore;
pub mod engine;
pub mod error;
pub mod event;
pub mod manifest;
pub mod resolve;

pub use buildx::{BuildxProcessBuildEngine, BuildxProcessOptions};
pub use dockerfile::{DockerfileSource, DockerfileValidationError, validate_dockerfile_source};
pub use dockerignore::{Dockerignore, DockerignorePattern};
pub use engine::{
    BoxBuildFuture, BuildCancellationToken, BuildCapabilities, BuildEngine, BuildImageIdentity,
    BuildRequest, BuildResult, BuildSecret, BuildSshForward, CacheEntry, InsecureEntitlements,
};
pub use error::{BuildError, BuildOperation};
pub use event::{
    BuildEvent, BuildEventSink, BuildId, BuildLogStream, BuildProgress, BuildVertexId,
    BuildVertexStatus,
};
pub use manifest::{BuildInputManifest, ManifestEntry, ManifestError};
pub use resolve::{BuildInputPaths, BuildResolveError, resolve_build_inputs};
