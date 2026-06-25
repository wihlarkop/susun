//! Runtime event model.

use std::{future::Future, pin::Pin, sync::Arc};

use susun_engine::ActionProgress;
use susun_planner::{ActionId, PlanId};

use crate::report::{ActionStatus, ExecutionSummary};

/// Runtime event.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", content = "payload", rename_all = "snake_case")
)]
pub enum RuntimeEvent {
    /// Plan execution started.
    PlanStarted {
        /// Plan ID.
        plan_id: PlanId,
    },
    /// Action is ready to run.
    ActionQueued {
        /// Action ID.
        action_id: ActionId,
    },
    /// Action started.
    ActionStarted {
        /// Action ID.
        action_id: ActionId,
    },
    /// Action progress.
    ActionProgress {
        /// Action ID.
        action_id: ActionId,
        /// Progress payload.
        progress: ActionProgress,
    },
    /// Action finished.
    ActionFinished {
        /// Action ID.
        action_id: ActionId,
        /// Status.
        status: ActionStatus,
    },
    /// Plan finished.
    PlanFinished {
        /// Plan ID.
        plan_id: PlanId,
        /// Summary.
        summary: ExecutionSummary,
    },
}

type EventFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Non-blocking event sink.
#[derive(Clone)]
pub struct EventSink {
    handler: Arc<dyn Fn(RuntimeEvent) -> EventFuture + Send + Sync>,
}

impl EventSink {
    /// Creates an event sink.
    pub fn new(handler: impl Fn(RuntimeEvent) -> EventFuture + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Creates a sink that drops all events.
    pub fn discard() -> Self {
        Self::new(|_| Box::pin(async {}))
    }

    /// Emits an event.
    pub async fn emit(&self, event: RuntimeEvent) {
        (self.handler)(event).await;
    }
}

impl std::fmt::Debug for EventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSink").finish_non_exhaustive()
    }
}
