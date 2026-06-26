//! Neutral build progress events.

use std::{future::Future, pin::Pin, sync::Arc};

/// Build identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildId(pub String);

/// Build graph vertex identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildVertexId(pub String);

/// Build log stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildLogStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Build vertex status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildVertexStatus {
    /// Vertex completed successfully.
    Succeeded,
    /// Vertex failed.
    Failed,
    /// Vertex was cancelled.
    Cancelled,
}

/// Byte progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProgress {
    /// Completed units.
    pub current: u64,
    /// Optional total units.
    pub total: Option<u64>,
}

/// Neutral build event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEvent {
    /// Build started.
    Started {
        /// Build ID.
        build_id: BuildId,
    },
    /// Build vertex started.
    VertexStarted {
        /// Vertex ID.
        vertex: BuildVertexId,
        /// Redacted vertex name.
        name: String,
    },
    /// Vertex progress update.
    VertexProgress {
        /// Vertex ID.
        vertex: BuildVertexId,
        /// Progress payload.
        progress: BuildProgress,
    },
    /// Vertex log line.
    VertexLog {
        /// Vertex ID.
        vertex: BuildVertexId,
        /// Log stream.
        stream: BuildLogStream,
        /// Redacted text.
        text: String,
    },
    /// Vertex finished.
    VertexFinished {
        /// Vertex ID.
        vertex: BuildVertexId,
        /// Status.
        status: BuildVertexStatus,
    },
    /// Build finished.
    Finished,
}

type BuildEventFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Non-blocking build event sink.
#[derive(Clone)]
pub struct BuildEventSink {
    handler: Arc<dyn Fn(BuildEvent) -> BuildEventFuture + Send + Sync>,
}

impl BuildEventSink {
    /// Creates an event sink.
    pub fn new(handler: impl Fn(BuildEvent) -> BuildEventFuture + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Creates a sink that drops all events.
    pub fn discard() -> Self {
        Self::new(|_| Box::pin(async {}))
    }

    /// Emits an event.
    pub async fn emit(&self, event: BuildEvent) {
        (self.handler)(event).await;
    }
}

impl std::fmt::Debug for BuildEventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildEventSink").finish_non_exhaustive()
    }
}
