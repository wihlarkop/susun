//! SDK runtime status summaries derived from neutral engine snapshots.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::Error as _};
use susun_engine::{
    ContainerState, EngineSnapshot, HealthState, ObservedContainer, ObservedImageRef,
    ProjectIdentity, RuntimeDoctorReport, RuntimeDoctorStatus, SnapshotCompleteness,
};

/// Combined runtime readiness and project status summary for dashboards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOverview {
    /// Serialized runtime overview schema version.
    pub schema_version: RuntimeOverviewSchemaVersion,
    /// Aggregate dashboard status.
    pub overview_status: RuntimeOverviewStatus,
    /// Runtime readiness report.
    pub doctor: RuntimeDoctorReport,
    /// Project status when a snapshot was available.
    pub status: Option<RuntimeStatusSummary>,
}

/// Serialized runtime overview schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeOverviewSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl RuntimeOverviewSchemaVersion {
    /// Current runtime overview schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Aggregate runtime overview status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOverviewStatus {
    /// Runtime is available and project status was produced.
    Ready,
    /// Runtime is available but project status was not produced.
    Degraded,
    /// Runtime is not available.
    Unavailable,
}

/// Serializable project runtime status summary for SDK, CLI, and UI consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatusSummary {
    /// Serialized runtime status summary schema version.
    pub schema_version: RuntimeStatusSummarySchemaVersion,
    /// Compose project name.
    pub project_name: String,
    /// Opaque project instance ID.
    pub project_instance: String,
    /// Resource counts for the selected project.
    pub counts: RuntimeResourceCountSummary,
    /// Per-service summaries, sorted by service name.
    pub services: Vec<RuntimeServiceStatusSummary>,
    /// Containers without service ownership, sorted by name.
    pub unassigned_containers: Vec<RuntimeContainerStatusSummary>,
}

/// Serialized runtime status summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatusSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl RuntimeStatusSummarySchemaVersion {
    /// Current runtime status summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Runtime resource counts for one project instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeResourceCountSummary {
    /// Number of observed containers.
    pub containers: usize,
    /// Number of running containers.
    pub running_containers: usize,
    /// Number of exited containers.
    pub exited_containers: usize,
    /// Number of paused containers.
    pub paused_containers: usize,
    /// Number of restarting containers.
    pub restarting_containers: usize,
    /// Number of observed networks.
    pub networks: usize,
    /// Number of observed volumes.
    pub volumes: usize,
}

/// Runtime status summary for one service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeServiceStatusSummary {
    /// Service name.
    pub service: String,
    /// Number of observed containers for this service.
    pub container_count: usize,
    /// Number of running containers for this service.
    pub running_containers: usize,
    /// Per-container summaries, sorted by replica then name.
    pub containers: Vec<RuntimeContainerStatusSummary>,
}

/// Runtime status summary for one container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContainerStatusSummary {
    /// Engine container ID.
    pub id: String,
    /// Runtime container name.
    pub name: String,
    /// Service name when ownership is known.
    pub service: Option<String>,
    /// One-based replica ordinal when ownership is known.
    pub replica: Option<u32>,
    /// Runtime container state.
    pub state: ContainerState,
    /// Optional health state.
    pub health: Option<HealthState>,
    /// Display-safe image reference.
    pub image: String,
    /// Completeness of the observed container record.
    pub completeness: SnapshotCompleteness,
}

/// Builds a runtime status summary for one project from a neutral snapshot.
pub fn runtime_status_from_snapshot(
    project: &ProjectIdentity,
    snapshot: &EngineSnapshot,
) -> RuntimeStatusSummary {
    let mut counts = RuntimeResourceCountSummary::default();
    let mut services: BTreeMap<String, Vec<RuntimeContainerStatusSummary>> = BTreeMap::new();
    let mut unassigned_containers = Vec::new();

    for container in snapshot
        .containers
        .values()
        .filter(|container| container_belongs_to_project(project, container))
    {
        counts.containers += 1;
        match container.state {
            ContainerState::Running => counts.running_containers += 1,
            ContainerState::Exited => counts.exited_containers += 1,
            ContainerState::Paused => counts.paused_containers += 1,
            ContainerState::Restarting => counts.restarting_containers += 1,
            ContainerState::Created | ContainerState::Unknown => {}
        }

        let summary = RuntimeContainerStatusSummary::from_container(container);
        match &summary.service {
            Some(service) => services.entry(service.clone()).or_default().push(summary),
            None => unassigned_containers.push(summary),
        }
    }

    counts.networks = snapshot
        .networks
        .values()
        .filter(|network| network.project_identity.as_ref() == Some(&project.working_set))
        .count();
    counts.volumes = snapshot
        .volumes
        .values()
        .filter(|volume| volume.project_identity.as_ref() == Some(&project.working_set))
        .count();

    let mut services = services
        .into_iter()
        .map(|(service, mut containers)| {
            sort_containers(&mut containers);
            let running_containers = containers
                .iter()
                .filter(|container| container.state == ContainerState::Running)
                .count();
            RuntimeServiceStatusSummary {
                service,
                container_count: containers.len(),
                running_containers,
                containers,
            }
        })
        .collect::<Vec<_>>();
    services.sort_by(|left, right| left.service.cmp(&right.service));
    sort_containers(&mut unassigned_containers);

    RuntimeStatusSummary {
        schema_version: RuntimeStatusSummarySchemaVersion::CURRENT,
        project_name: project.name.as_str().to_owned(),
        project_instance: project.working_set.as_str().to_owned(),
        counts,
        services,
        unassigned_containers,
    }
}

/// Builds a runtime overview from a doctor report and optional project status.
pub fn runtime_overview(
    doctor: RuntimeDoctorReport,
    status: Option<RuntimeStatusSummary>,
) -> RuntimeOverview {
    let overview_status = match (doctor.status, status.is_some()) {
        (RuntimeDoctorStatus::Available, true) => RuntimeOverviewStatus::Ready,
        (RuntimeDoctorStatus::Available, false) => RuntimeOverviewStatus::Degraded,
        _ => RuntimeOverviewStatus::Unavailable,
    };
    RuntimeOverview {
        schema_version: RuntimeOverviewSchemaVersion::CURRENT,
        overview_status,
        doctor,
        status,
    }
}

/// Renders a runtime overview as pretty JSON.
pub fn render_runtime_overview_json(
    overview: &RuntimeOverview,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(overview)
}

/// Parses a runtime overview from JSON.
pub fn parse_runtime_overview_json(input: &str) -> Result<RuntimeOverview, serde_json::Error> {
    let overview: RuntimeOverview = serde_json::from_str(input)?;
    validate_runtime_overview_schema(&overview)?;
    Ok(overview)
}

/// Renders a runtime status summary as pretty JSON.
pub fn render_runtime_status_summary_json(
    summary: &RuntimeStatusSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a runtime status summary from JSON.
pub fn parse_runtime_status_summary_json(
    input: &str,
) -> Result<RuntimeStatusSummary, serde_json::Error> {
    let summary: RuntimeStatusSummary = serde_json::from_str(input)?;
    validate_runtime_status_summary_schema(&summary)?;
    Ok(summary)
}

fn validate_runtime_overview_schema(overview: &RuntimeOverview) -> Result<(), serde_json::Error> {
    if overview.schema_version != RuntimeOverviewSchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported runtime overview schema version {}.{}",
            overview.schema_version.major, overview.schema_version.minor
        )));
    }
    let expected_status = match (overview.doctor.status, overview.status.is_some()) {
        (RuntimeDoctorStatus::Available, true) => RuntimeOverviewStatus::Ready,
        (RuntimeDoctorStatus::Available, false) => RuntimeOverviewStatus::Degraded,
        _ => RuntimeOverviewStatus::Unavailable,
    };
    if overview.overview_status != expected_status {
        return Err(serde_json::Error::custom(
            "runtime overview status does not match doctor and project status",
        ));
    }
    if let Some(status) = &overview.status {
        validate_runtime_status_summary_schema(status)?;
    }
    Ok(())
}

fn validate_runtime_status_summary_schema(
    summary: &RuntimeStatusSummary,
) -> Result<(), serde_json::Error> {
    if summary.schema_version != RuntimeStatusSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported runtime status summary schema version {}.{}",
            summary.schema_version.major, summary.schema_version.minor
        )));
    }
    if summary.project_name.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "runtime status project name must not be empty",
        ));
    }
    if summary.project_instance.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "runtime status project instance must not be empty",
        ));
    }
    let mut container_count = summary.unassigned_containers.len();
    let mut running_containers = 0usize;
    let mut exited_containers = 0usize;
    let mut paused_containers = 0usize;
    let mut restarting_containers = 0usize;
    let mut service_names = std::collections::BTreeSet::new();

    for container in &summary.unassigned_containers {
        validate_runtime_container_summary(container)?;
        if container.service.is_some() {
            return Err(serde_json::Error::custom(
                "runtime status unassigned container has service identity",
            ));
        }
        count_container_state(
            container,
            &mut running_containers,
            &mut exited_containers,
            &mut paused_containers,
            &mut restarting_containers,
        );
    }
    for service in &summary.services {
        if service.service.trim().is_empty() {
            return Err(serde_json::Error::custom(
                "runtime status service name must not be empty",
            ));
        }
        if !service_names.insert(service.service.as_str()) {
            return Err(serde_json::Error::custom(
                "runtime status contains duplicate service summaries",
            ));
        }
        if service.container_count != service.containers.len() {
            return Err(serde_json::Error::custom(
                "runtime status service container count does not match containers",
            ));
        }
        let running = service
            .containers
            .iter()
            .filter(|container| container.state == ContainerState::Running)
            .count();
        if service.running_containers != running {
            return Err(serde_json::Error::custom(
                "runtime status service running count does not match containers",
            ));
        }
        for container in &service.containers {
            validate_runtime_container_summary(container)?;
            if container.service.as_deref() != Some(service.service.as_str()) {
                return Err(serde_json::Error::custom(
                    "runtime status service container service identity does not match service",
                ));
            }
            count_container_state(
                container,
                &mut running_containers,
                &mut exited_containers,
                &mut paused_containers,
                &mut restarting_containers,
            );
        }
        container_count += service.containers.len();
    }
    if summary.counts.containers != container_count
        || summary.counts.running_containers != running_containers
        || summary.counts.exited_containers != exited_containers
        || summary.counts.paused_containers != paused_containers
        || summary.counts.restarting_containers != restarting_containers
    {
        return Err(serde_json::Error::custom(
            "runtime status container counts do not match containers",
        ));
    }
    Ok(())
}

fn validate_runtime_container_summary(
    container: &RuntimeContainerStatusSummary,
) -> Result<(), serde_json::Error> {
    if container.id.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "runtime status container id must not be empty",
        ));
    }
    if container.name.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "runtime status container name must not be empty",
        ));
    }
    if container.image.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "runtime status container image must not be empty",
        ));
    }
    if container
        .service
        .as_deref()
        .is_some_and(|service| service.trim().is_empty())
    {
        return Err(serde_json::Error::custom(
            "runtime status container service must not be empty",
        ));
    }
    if container.replica == Some(0) {
        return Err(serde_json::Error::custom(
            "runtime status container replica must be one-based",
        ));
    }
    Ok(())
}

fn count_container_state(
    container: &RuntimeContainerStatusSummary,
    running_containers: &mut usize,
    exited_containers: &mut usize,
    paused_containers: &mut usize,
    restarting_containers: &mut usize,
) {
    match container.state {
        ContainerState::Running => *running_containers += 1,
        ContainerState::Exited => *exited_containers += 1,
        ContainerState::Paused => *paused_containers += 1,
        ContainerState::Restarting => *restarting_containers += 1,
        ContainerState::Created | ContainerState::Unknown => {}
    }
}

impl RuntimeContainerStatusSummary {
    fn from_container(container: &ObservedContainer) -> Self {
        let service = container
            .service_identity
            .as_ref()
            .map(|identity| identity.service.as_str().to_owned());
        let replica = container
            .service_identity
            .as_ref()
            .map(|identity| identity.replica.ordinal());
        Self {
            id: container.id.as_str().to_owned(),
            name: container.name.as_str().to_owned(),
            service,
            replica,
            state: container.state,
            health: container.health,
            image: image_summary(&container.image),
            completeness: container.completeness.clone(),
        }
    }
}

fn container_belongs_to_project(project: &ProjectIdentity, container: &ObservedContainer) -> bool {
    if container.project_identity.as_ref() == Some(&project.working_set) {
        return true;
    }
    container
        .service_identity
        .as_ref()
        .is_some_and(|identity| identity.project == project.working_set)
}

fn image_summary(image: &ObservedImageRef) -> String {
    match image {
        ObservedImageRef::Id(id) => id.as_str().to_owned(),
        ObservedImageRef::Reference(reference) => reference.as_str().to_owned(),
        ObservedImageRef::Unknown => "<unknown>".to_owned(),
    }
}

fn sort_containers(containers: &mut [RuntimeContainerStatusSummary]) {
    containers.sort_by(|left, right| {
        (
            left.replica.unwrap_or(u32::MAX),
            left.service.as_deref().unwrap_or(""),
            left.name.as_str(),
            left.id.as_str(),
        )
            .cmp(&(
                right.replica.unwrap_or(u32::MAX),
                right.service.as_deref().unwrap_or(""),
                right.name.as_str(),
                right.id.as_str(),
            ))
    });
}
