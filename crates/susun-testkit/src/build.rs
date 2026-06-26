//! Fake build engine for tests and compatibility fixtures.

use std::sync::{Arc, Mutex};

use susun_build::{
    BoxBuildFuture, BuildCancellationToken, BuildCapabilities, BuildEngine, BuildError,
    BuildEventSink, BuildImageIdentity, BuildRequest, BuildResult,
};

/// In-memory fake build engine.
#[derive(Debug, Clone)]
pub struct FakeBuildEngine {
    capabilities: BuildCapabilities,
    image: BuildImageIdentity,
    requests: Arc<Mutex<Vec<BuildRequest>>>,
}

impl FakeBuildEngine {
    /// Creates a fake engine returning `image`.
    pub fn new(image: BuildImageIdentity) -> Self {
        Self {
            capabilities: BuildCapabilities::buildx_process(),
            image,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns recorded build requests.
    pub fn requests(&self) -> Vec<BuildRequest> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

impl BuildEngine for FakeBuildEngine {
    fn capabilities(&self) -> BoxBuildFuture<'_, BuildCapabilities> {
        let capabilities = self.capabilities.clone();
        Box::pin(async move { Ok(capabilities) })
    }

    fn build(
        &self,
        request: BuildRequest,
        _events: BuildEventSink,
        cancellation: BuildCancellationToken,
    ) -> BoxBuildFuture<'_, BuildResult> {
        let image = self.image.clone();
        let requests = self.requests.clone();
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(BuildError::Cancelled);
            }
            if let Ok(mut recorded) = requests.lock() {
                recorded.push(request);
            }
            Ok(BuildResult { image })
        })
    }
}
