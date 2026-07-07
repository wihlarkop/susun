#![allow(missing_docs)]

use std::time::SystemTime;

use susun::{
    ContainerId, ContainerState, EngineSnapshot, HealthState, ObservedContainer, ObservedImageRef,
    ProjectIdentity, ProjectInstanceId, ProjectName, ReplicaIndex, ResourceName, ServiceInstanceId,
    ServiceName, SnapshotCompleteness, parse_runtime_status_summary_json,
    render_runtime_status_summary_json, runtime_status_from_snapshot,
};
use susun_model::ImageRef;

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
    assert!(json.contains("\"project_name\""));
    assert!(json.contains("\"services\""));
    Ok(())
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
