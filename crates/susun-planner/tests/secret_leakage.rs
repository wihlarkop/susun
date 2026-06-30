//! Secret redaction coverage for serialized plan artifacts.

use std::error::Error;

use indexmap::IndexMap;
use susun_diagnostics::DiagnosticReport;
use susun_engine::{
    ProjectIdentity, ProjectInstanceId, ReplicaIndex, ResourceName, ServiceInstanceId,
};
use susun_model::{ImageRef, ProjectName, ServiceName};
use susun_planner::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, CreateContainerAction, ExecutionPlan,
    PlanAction, PlanActionNode, PlannedOperation,
};

#[test]
fn serialized_plan_redacts_sensitive_environment_values() -> Result<(), Box<dyn Error>> {
    let plan = plan_with_sensitive_environment()?;
    let json = serde_json::to_string(&plan)?;

    assert!(!json.contains("super-secret-token"));
    assert!(json.contains("<redacted>"));
    assert!(json.contains("API_TOKEN"));
    Ok(())
}

fn plan_with_sensitive_environment() -> Result<ExecutionPlan, Box<dyn Error>> {
    let project = ProjectName::new("secret-fixture");
    let working_set = ProjectInstanceId::derive(&project, ".");
    let service = ServiceName::new("web");
    let identity = ServiceInstanceId::new(working_set.clone(), service, ReplicaIndex::FIRST);
    let action_id = ActionId::from_parts(&["create", "web"]);
    let mut environment = IndexMap::new();
    environment.insert(
        "API_TOKEN".to_owned(),
        Some("super-secret-token".to_owned()),
    );
    environment.insert("PUBLIC_MODE".to_owned(), Some("demo".to_owned()));

    let mut actions = IndexMap::new();
    actions.insert(
        action_id.clone(),
        PlanActionNode {
            id: action_id,
            action: PlanAction::CreateContainer(Box::new(CreateContainerAction {
                identity,
                name: ResourceName::new("secret-fixture-web-1")?,
                image: Some(ImageRef::new("busybox:latest")),
                command: None,
                entrypoint: None,
                environment,
                labels: IndexMap::new(),
                ports: Vec::new(),
                volumes: Vec::new(),
                configs: Vec::new(),
                secrets: Vec::new(),
                networks: IndexMap::new(),
                healthcheck: None,
                restart: None,
            })),
            dependencies: Default::default(),
            reason: ActionExplanation::new(
                ActionReason::ServiceRequested,
                "selected service requires a container",
            ),
            safety: ActionSafety::Safe,
        },
    );

    Ok(ExecutionPlan::new(
        ProjectIdentity::new(project, working_set),
        PlannedOperation::Up,
        actions,
        DiagnosticReport::new(),
    ))
}
