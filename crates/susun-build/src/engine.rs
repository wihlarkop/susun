//! Neutral build engine contract.

use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use indexmap::IndexMap;
use susun_model::BuildDefinition;

use crate::{BuildError, BuildEventSink, BuildInputManifest};

/// Boxed build future.
pub type BoxBuildFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BuildError>> + Send + 'a>>;

/// Supported build capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildCapabilities {
    /// Whether target stages are supported.
    pub targets: bool,
    /// Whether build args are supported.
    pub args: bool,
    /// Whether multi-platform requests are supported.
    pub platforms: bool,
    /// Whether build secrets are supported.
    pub secrets: bool,
    /// Whether SSH forwarding is supported.
    pub ssh: bool,
    /// Whether cache import/export is supported.
    pub cache: bool,
    /// Whether insecure entitlements are supported.
    pub insecure_entitlements: bool,
}

impl BuildCapabilities {
    /// Capabilities supported by the buildx process adapter.
    pub fn buildx_process() -> Self {
        Self {
            targets: true,
            args: true,
            platforms: true,
            secrets: true,
            ssh: true,
            cache: true,
            insecure_entitlements: true,
        }
    }
}

/// Cooperative build cancellation token.
#[derive(Debug, Clone, Default)]
pub struct BuildCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl BuildCancellationToken {
    /// Creates a token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Neutral build engine.
pub trait BuildEngine: Send + Sync {
    /// Returns build capabilities.
    fn capabilities(&self) -> BoxBuildFuture<'_, BuildCapabilities>;

    /// Executes a build.
    fn build(
        &self,
        request: BuildRequest,
        events: BuildEventSink,
        cancellation: BuildCancellationToken,
    ) -> BoxBuildFuture<'_, BuildResult>;
}

/// Neutral build request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    /// Compose build definition.
    pub definition: BuildDefinition,
    /// Resolved context directory.
    pub context_dir: PathBuf,
    /// Resolved Dockerfile path.
    pub dockerfile: PathBuf,
    /// Deterministic input manifest.
    pub manifest: BuildInputManifest,
    /// Image tag to load or export.
    pub image_tag: Option<String>,
    /// Build secrets by identity only. Values are provider-owned and must not
    /// serialize into plans.
    pub secrets: Vec<BuildSecret>,
    /// SSH forwarding identities.
    pub ssh: Vec<BuildSshForward>,
    /// Cache imports.
    pub cache_from: Vec<CacheEntry>,
    /// Cache exports.
    pub cache_to: Vec<CacheEntry>,
    /// Explicitly enabled insecure entitlements.
    pub insecure_entitlements: InsecureEntitlements,
    /// Additional build labels.
    pub labels: IndexMap<String, String>,
}

/// Build secret reference.
#[derive(Clone, PartialEq, Eq)]
pub struct BuildSecret {
    /// Secret ID.
    pub id: String,
    /// Optional provider-owned source path.
    pub source: Option<PathBuf>,
}

impl std::fmt::Debug for BuildSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildSecret")
            .field("id", &self.id)
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// SSH forwarding reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSshForward {
    /// SSH ID.
    pub id: String,
}

/// Cache entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    /// Cache spec, for example `type=registry,ref=example/cache`.
    pub spec: String,
}

/// Insecure entitlements requiring explicit opt-in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InsecureEntitlements {
    /// Allow host network during build.
    pub network_host: bool,
    /// Allow security insecure entitlement.
    pub security_insecure: bool,
}

/// Built image identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildImageIdentity {
    /// Image reference or tag.
    pub reference: String,
    /// Optional immutable digest.
    pub digest: Option<String>,
}

/// Build result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildResult {
    /// Resulting image identity.
    pub image: BuildImageIdentity,
}
