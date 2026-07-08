//! Fake engine failure matrix coverage.

use std::{error::Error, sync::Arc};

use indexmap::IndexMap;
use susun_engine::{EngineOperation, ProjectIdentity, ProjectInstanceId};
use susun_model::{ImageRef, ProjectName};
use susun_planner::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, ExecutionPlan, PlanAction,
    PlanActionNode, PlannedOperation, PullImageAction,
};
use susun_runtime::{ActionStatus, Runtime, RuntimeError};
use susun_testkit::FakeContainerEngine;

#[tokio::test]
async fn capability_failure_stops_before_report() -> Result<(), Box<dyn Error>> {
    let engine = FakeContainerEngine::failing(EngineOperation::Capabilities);
    let error = Runtime::new(Arc::new(engine)).apply(&pull_plan()).await;

    assert!(matches!(error, Err(RuntimeError::Capabilities(_))));
    Ok(())
}

#[tokio::test]
async fn snapshot_failure_stops_before_report() -> Result<(), Box<dyn Error>> {
    let engine = FakeContainerEngine::failing(EngineOperation::Snapshot);
    let error = Runtime::new(Arc::new(engine)).apply(&pull_plan()).await;

    assert!(matches!(error, Err(RuntimeError::Capabilities(_))));
    Ok(())
}

#[tokio::test]
async fn action_failure_is_recorded_in_report() -> Result<(), Box<dyn Error>> {
    let action_id = action_id();
    let engine = FakeContainerEngine::failing(EngineOperation::PullImage);
    let report = Runtime::new(Arc::new(engine)).apply(&pull_plan()).await?;
    let Some(result) = report.actions.get(&action_id) else {
        return Err("missing pull action result".into());
    };

    assert_eq!(result.status, ActionStatus::Failed);
    assert_eq!(result.attempts, 2);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.summary.succeeded, 0);
    assert_eq!(result.error.as_deref(), Some("engine pull image failed"));
    Ok(())
}

fn pull_plan() -> ExecutionPlan {
    let mut actions = IndexMap::new();
    let action_id = action_id();
    actions.insert(
        action_id.clone(),
        PlanActionNode {
            id: action_id,
            action: PlanAction::PullImage(PullImageAction {
                image: ImageRef::new("example.com/app:latest"),
            }),
            dependencies: Default::default(),
            reason: ActionExplanation::new(
                ActionReason::ImageUnavailableLocally,
                "image is unavailable locally",
            ),
            safety: ActionSafety::Safe,
        },
    );

    let project = ProjectName::new("fake-engine");
    ExecutionPlan::new(
        ProjectIdentity::new(project.clone(), ProjectInstanceId::derive(&project, ".")),
        PlannedOperation::Up,
        actions,
        Default::default(),
    )
}

fn action_id() -> ActionId {
    ActionId::from_parts(&["pull", "example.com/app:latest"])
}
