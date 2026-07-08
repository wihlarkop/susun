#![allow(missing_docs)]

use std::{path::PathBuf, sync::Arc, time::SystemTime};

use susun::{
    BuildPolicy, ContainerId, ContainerState, EngineSnapshot, HealthState, ObservedContainer,
    ObservedImageRef, ProjectIdentity, ProjectInstanceId, ProjectName, ReplicaIndex, ResourceName,
    RuntimeOperationError, ServiceInstanceId, ServiceName, SnapshotCompleteness, SusunWorkspace,
    UpPlanOptions, parse_runtime_operation_result_json, parse_runtime_operation_summary_json,
    parse_runtime_overview_json, parse_runtime_status_summary_json,
    render_runtime_operation_result_json, render_runtime_operation_summary_json,
    render_runtime_overview_json, render_runtime_status_summary_json, runtime_overview,
    runtime_status_from_snapshot,
};
use susun::{
    RuntimeDoctorReport, RuntimeDoctorStatus, RuntimeOperationSummary,
    RuntimeOperationSummarySchemaVersion, RuntimeOverviewSchemaVersion, RuntimeOverviewStatus,
    RuntimeStatusSummarySchemaVersion,
};
use susun_engine::EngineOperation;
use susun_model::ImageRef;
use susun_testkit::FakeContainerEngine;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn runtime_status_filters_and_groups_project_containers() -> TestResult {
    let project = project_identity("app", "project-a")?;
    let foreign_project = project_identity("other", "project-b")?;
    let mut snapshot = EngineSnapshot::empty(SystemTime::UNIX_EPOCH);

    let web_one = container(
        "container-1",
        "app-web-1",
        &project,
        Some(("web", 0)),
        ContainerState::Running,
    )?;
    let web_two = container(
        "container-2",
        "app-web-2",
        &project,
        Some(("web", 1)),
        ContainerState::Exited,
    )?;
    let unassigned = container(
        "container-3",
        "app-unassigned",
        &project,
        None,
        ContainerState::Paused,
    )?;
    let foreign = container(
        "container-4",
        "other-web-1",
        &foreign_project,
        Some(("web", 0)),
        ContainerState::Running,
    )?;

    snapshot.containers.insert(web_two.id.clone(), web_two);
    snapshot.containers.insert(foreign.id.clone(), foreign);
    snapshot
        .containers
        .insert(unassigned.id.clone(), unassigned);
    snapshot.containers.insert(web_one.id.clone(), web_one);

    let summary = runtime_status_from_snapshot(&project, &snapshot);

    assert_eq!(
        summary.schema_version,
        RuntimeStatusSummarySchemaVersion::CURRENT
    );
    assert_eq!(summary.project_name, "app");
    assert_eq!(summary.project_instance, "project-a");
    assert_eq!(summary.counts.containers, 3);
    assert_eq!(summary.counts.running_containers, 1);
    assert_eq!(summary.counts.exited_containers, 1);
    assert_eq!(summary.counts.paused_containers, 1);
    assert_eq!(summary.services.len(), 1);
    assert_eq!(summary.services[0].service, "web");
    assert_eq!(summary.services[0].container_count, 2);
    assert_eq!(summary.services[0].running_containers, 1);
    assert_eq!(summary.services[0].containers[0].replica, Some(1));
    assert_eq!(summary.services[0].containers[1].replica, Some(2));
    assert_eq!(summary.unassigned_containers.len(), 1);
    assert_eq!(summary.unassigned_containers[0].name, "app-unassigned");
    Ok(())
}

#[test]
fn runtime_status_json_helpers_roundtrip() -> TestResult {
    let project = project_identity("app", "project-a")?;
    let mut snapshot = EngineSnapshot::empty(SystemTime::UNIX_EPOCH);
    let container = container(
        "container-1",
        "app-web-1",
        &project,
        Some(("web", 0)),
        ContainerState::Running,
    )?;
    snapshot.containers.insert(container.id.clone(), container);

    let summary = runtime_status_from_snapshot(&project, &snapshot);
    let json = render_runtime_status_summary_json(&summary)?;
    let parsed = parse_runtime_status_summary_json(&json)?;

    assert_eq!(parsed, summary);
    assert_eq!(
        parsed.schema_version,
        RuntimeStatusSummarySchemaVersion::CURRENT
    );
    assert!(json.contains("\"project_name\""));
    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"services\""));
    Ok(())
}

#[test]
fn sdk_project_runtime_status_from_snapshot_uses_project_identity() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let identity = sdk_project.identity().ok_or("expected project identity")?;
    let foreign_project = project_identity("other", "project-b")?;
    let mut snapshot = EngineSnapshot::empty(SystemTime::UNIX_EPOCH);

    let project_container = container(
        "container-1",
        "valid-minimal-web-1",
        identity,
        Some(("web", 0)),
        ContainerState::Running,
    )?;
    let foreign_container = container(
        "container-2",
        "other-web-1",
        &foreign_project,
        Some(("web", 0)),
        ContainerState::Running,
    )?;
    snapshot
        .containers
        .insert(project_container.id.clone(), project_container);
    snapshot
        .containers
        .insert(foreign_container.id.clone(), foreign_container);

    let summary = sdk_project
        .runtime_status_from_snapshot(&snapshot)
        .ok_or("expected runtime status")?;

    assert_eq!(summary.project_name, "valid-minimal");
    assert_eq!(summary.counts.containers, 1);
    assert_eq!(summary.services[0].service, "web");
    Ok(())
}

#[tokio::test]
async fn sdk_project_runtime_status_with_engine_uses_supplied_engine() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let identity = sdk_project.identity().ok_or("expected project identity")?;
    let mut snapshot = EngineSnapshot::empty(SystemTime::UNIX_EPOCH);
    let project_container = container(
        "container-1",
        "valid-minimal-web-1",
        identity,
        Some(("web", 0)),
        ContainerState::Running,
    )?;
    snapshot
        .containers
        .insert(project_container.id.clone(), project_container);
    let engine = FakeContainerEngine::new().with_snapshot(snapshot);

    let summary = sdk_project
        .runtime_status_with_engine(&engine)
        .await?
        .ok_or("expected runtime status")?;

    assert_eq!(summary.counts.running_containers, 1);
    assert_eq!(summary.services[0].container_count, 1);
    Ok(())
}

#[tokio::test]
async fn sdk_project_runtime_status_with_engine_returns_snapshot_errors() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let engine = FakeContainerEngine::failing(EngineOperation::Snapshot);

    let error = sdk_project.runtime_status_with_engine(&engine).await;

    assert!(error.is_err());
    Ok(())
}

#[tokio::test]
async fn sdk_project_runtime_overview_skips_snapshot_when_doctor_unavailable() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let doctor = RuntimeDoctorReport {
        profile_id: None,
        status: RuntimeDoctorStatus::Unavailable,
        endpoint: susun::RedactedEndpoint::new(&susun::EngineEndpoint::Local),
        probe: None,
        message: "runtime unavailable".to_owned(),
    };
    let engine = FakeContainerEngine::failing(EngineOperation::Snapshot);

    let overview = sdk_project
        .runtime_overview_with_engine(doctor, &engine)
        .await?;

    assert_eq!(overview.overview_status, RuntimeOverviewStatus::Unavailable);
    assert!(overview.status.is_none());
    Ok(())
}

#[tokio::test]
async fn sdk_project_plan_up_with_engine_returns_plan_outcome() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let engine = FakeContainerEngine::new();
    let options = UpPlanOptions {
        build_policy: BuildPolicy::NeverBuild,
        ..UpPlanOptions::default()
    };

    let outcome = sdk_project.plan_up_with_engine(&engine, options).await?;

    assert!(!outcome.diagnostics.has_errors());
    assert!(outcome.plan.is_some());
    Ok(())
}

#[tokio::test]
async fn sdk_project_plan_up_with_engine_returns_snapshot_errors() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let engine = FakeContainerEngine::failing(EngineOperation::Snapshot);

    let error = sdk_project
        .plan_up_with_engine(&engine, UpPlanOptions::default())
        .await;

    assert!(matches!(error, Err(RuntimeOperationError::Engine(_))));
    Ok(())
}

#[tokio::test]
async fn sdk_project_up_with_engine_executes_through_runtime_facade() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let engine = Arc::new(FakeContainerEngine::new());
    let options = UpPlanOptions {
        build_policy: BuildPolicy::NeverBuild,
        ..UpPlanOptions::default()
    };

    let result = sdk_project.up_with_engine(engine, options).await?;

    assert_eq!(result.plan.project.name.as_str(), "valid-minimal");
    assert!(result.report.summary.total_actions > 0);
    assert_eq!(result.report.summary.failed, 0);
    Ok(())
}

#[tokio::test]
async fn runtime_operation_result_json_helpers_roundtrip() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let engine = Arc::new(FakeContainerEngine::new());
    let options = UpPlanOptions {
        build_policy: BuildPolicy::NeverBuild,
        ..UpPlanOptions::default()
    };
    let result = sdk_project.up_with_engine(engine, options).await?;

    let json = render_runtime_operation_result_json(&result)?;
    let parsed = parse_runtime_operation_result_json(&json)?;

    assert_eq!(parsed.plan.plan_id, result.plan.plan_id);
    assert_eq!(parsed.report.plan_id, result.report.plan_id);
    assert_eq!(
        parsed.report.summary.total_actions,
        result.report.summary.total_actions
    );
    Ok(())
}

#[tokio::test]
async fn runtime_operation_summary_json_helpers_roundtrip() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let engine = Arc::new(FakeContainerEngine::new());
    let options = UpPlanOptions {
        build_policy: BuildPolicy::NeverBuild,
        ..UpPlanOptions::default()
    };
    let result = sdk_project.up_with_engine(engine, options).await?;
    let summary = RuntimeOperationSummary::from(&result);

    assert_eq!(
        summary.schema_version,
        RuntimeOperationSummarySchemaVersion::CURRENT
    );
    assert_eq!(summary.plan_id, result.plan.plan_id.as_str());
    assert_eq!(summary.operation, "up");
    assert_eq!(summary.planned_actions, result.plan.summary.total_actions);
    assert_eq!(
        summary.reported_actions,
        result.report.summary.total_actions
    );
    assert_eq!(summary.failed, 0);

    let json = render_runtime_operation_summary_json(&summary)?;
    let parsed = parse_runtime_operation_summary_json(&json)?;

    assert_eq!(parsed, summary);
    Ok(())
}

#[tokio::test]
async fn sdk_project_up_with_engine_requires_analyzed_project() -> TestResult {
    let sdk_project = SusunWorkspace::from_file(malformed_path()).analyze()?;
    let engine = Arc::new(FakeContainerEngine::failing(EngineOperation::Capabilities));

    let error = sdk_project
        .up_with_engine(engine, UpPlanOptions::default())
        .await;

    assert!(matches!(error, Err(RuntimeOperationError::MissingProject)));
    Ok(())
}

#[test]
fn runtime_overview_is_ready_when_doctor_and_status_available() -> TestResult {
    let project = project_identity("app", "project-a")?;
    let snapshot = EngineSnapshot::empty(SystemTime::UNIX_EPOCH);
    let status = runtime_status_from_snapshot(&project, &snapshot);
    let doctor = RuntimeDoctorReport::available(
        None,
        &susun::EngineEndpoint::Local,
        susun::EngineProbe {
            api_version: None,
            engine_version: None,
            operating_system: None,
            architecture: None,
        },
    );

    let overview = runtime_overview(doctor, Some(status));

    assert_eq!(
        overview.schema_version,
        RuntimeOverviewSchemaVersion::CURRENT
    );
    assert_eq!(overview.overview_status, RuntimeOverviewStatus::Ready);
    assert!(overview.status.is_some());
    Ok(())
}

#[test]
fn runtime_overview_is_degraded_when_status_missing() {
    let doctor = RuntimeDoctorReport::available(
        None,
        &susun::EngineEndpoint::Local,
        susun::EngineProbe {
            api_version: None,
            engine_version: None,
            operating_system: None,
            architecture: None,
        },
    );

    let overview = runtime_overview(doctor, None);

    assert_eq!(overview.overview_status, RuntimeOverviewStatus::Degraded);
}

#[test]
fn runtime_overview_json_helpers_roundtrip() -> TestResult {
    let doctor = RuntimeDoctorReport::available(
        None,
        &susun::EngineEndpoint::Local,
        susun::EngineProbe {
            api_version: None,
            engine_version: None,
            operating_system: None,
            architecture: None,
        },
    );
    let overview = runtime_overview(doctor, None);

    let json = render_runtime_overview_json(&overview)?;
    let parsed = parse_runtime_overview_json(&json)?;

    assert_eq!(parsed, overview);
    assert_eq!(parsed.schema_version, RuntimeOverviewSchemaVersion::CURRENT);
    assert!(json.contains("\"overview_status\""));
    assert!(json.contains("\"schema_version\""));
    Ok(())
}

fn valid_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/valid-minimal/compose.yaml")
}

fn malformed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/malformed/compose.yaml")
}

fn project_identity(
    name: &str,
    working_set: &str,
) -> Result<ProjectIdentity, Box<dyn std::error::Error>> {
    Ok(ProjectIdentity::new(
        ProjectName::new(name),
        ProjectInstanceId::new(working_set)?,
    ))
}

fn container(
    id: &str,
    name: &str,
    project: &ProjectIdentity,
    service: Option<(&str, u32)>,
    state: ContainerState,
) -> Result<ObservedContainer, Box<dyn std::error::Error>> {
    Ok(ObservedContainer {
        id: ContainerId::new(id)?,
        name: ResourceName::new(name)?,
        state,
        health: Some(HealthState::Healthy),
        image: ObservedImageRef::Reference(ImageRef::new("nginx:latest")),
        labels: Default::default(),
        project_identity: Some(project.working_set.clone()),
        service_identity: service.map(|(service, replica)| {
            ServiceInstanceId::new(
                project.working_set.clone(),
                ServiceName::new(service),
                ReplicaIndex::new(replica),
            )
        }),
        configuration_fingerprint: None,
        completeness: SnapshotCompleteness::Complete,
    })
}
