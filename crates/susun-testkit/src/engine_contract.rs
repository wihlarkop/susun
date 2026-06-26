//! Reusable neutral engine adapter contract checks.

use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use susun_engine::{
    ContainerEngine, CreateNetworkRequest, CreateVolumeRequest, EngineError, EngineOperation,
    LabelKey, LabelValue, NetworkRef, ProjectIdentity, ProjectInstanceId, ResourceName, VolumeRef,
};
use susun_model::ProjectName;

/// Project identity and runtime resource names for an adapter contract run.
#[derive(Debug, Clone)]
pub struct ContractProject {
    /// Project identity.
    pub identity: ProjectIdentity,
    /// Runtime network name.
    pub network_name: ResourceName,
    /// Runtime volume name.
    pub volume_name: ResourceName,
}

impl ContractProject {
    /// Creates a unique contract project from a caller-provided suffix.
    pub fn new(suffix: &str) -> Result<Self, EngineError> {
        let safe_suffix = suffix
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let suffix = safe_suffix.trim_matches('-');
        let suffix = if suffix.is_empty() {
            "contract"
        } else {
            suffix
        };
        let working_set = ProjectInstanceId::new(format!("contract-{suffix}"))
            .map_err(|error| EngineError::api(susun_engine::EngineOperation::Snapshot, error))?;
        let identity = ProjectIdentity::new(ProjectName::new("contract"), working_set);
        Ok(Self {
            network_name: resource_name(format!("susun-contract-{suffix}-net"))?,
            volume_name: resource_name(format!("susun-contract-{suffix}-vol"))?,
            identity,
        })
    }
}

/// Verifies basic non-mutating engine behavior.
pub async fn assert_basic_engine_contract<E>(engine: &E) -> Result<(), EngineError>
where
    E: ContainerEngine,
{
    let _capabilities = engine.capabilities().await?;
    let project = ContractProject::new(&unique_suffix("basic"))?;
    let snapshot = engine.snapshot(&project.identity).await?;
    for container in snapshot.containers.values() {
        if container.project_identity.as_ref() != Some(&project.identity.working_set) {
            return contract_error("snapshot returned a container outside the requested project");
        }
    }
    for network in snapshot.networks.values() {
        if network.project_identity.as_ref() != Some(&project.identity.working_set) {
            return contract_error("snapshot returned a network outside the requested project");
        }
    }
    for volume in snapshot.volumes.values() {
        if volume.project_identity.as_ref() != Some(&project.identity.working_set) {
            return contract_error("snapshot returned a volume outside the requested project");
        }
    }
    Ok(())
}

/// Verifies create, observe, and remove for project-owned network and volume resources.
pub async fn assert_resource_lifecycle_contract<E>(
    engine: &E,
    project: &ContractProject,
) -> Result<ContractResources, EngineError>
where
    E: ContainerEngine,
{
    let labels = ownership_labels(project)?;
    let network = engine
        .create_network(CreateNetworkRequest {
            project: project.identity.clone(),
            name: project.network_name.clone(),
            labels: labels.clone(),
        })
        .await?;
    let volume = engine
        .create_volume(CreateVolumeRequest {
            project: project.identity.clone(),
            name: project.volume_name.clone(),
            labels,
        })
        .await?;

    let snapshot = engine.snapshot(&project.identity).await?;
    if !snapshot.networks.values().any(|network| {
        network.name == project.network_name
            && network.project_identity.as_ref() == Some(&project.identity.working_set)
    }) {
        return contract_error("created network was not visible in the project snapshot");
    }
    if !snapshot.volumes.values().any(|volume| {
        volume.name == project.volume_name
            && volume.project_identity.as_ref() == Some(&project.identity.working_set)
    }) {
        return contract_error("created volume was not visible in the project snapshot");
    }

    Ok(ContractResources { network, volume })
}

/// Created resources that callers must clean up.
#[derive(Debug, Clone)]
pub struct ContractResources {
    /// Created network reference.
    pub network: NetworkRef,
    /// Created volume reference.
    pub volume: VolumeRef,
}

/// Creates a deterministic ownership label set for contract-created resources.
pub fn ownership_labels(
    project: &ContractProject,
) -> Result<IndexMap<LabelKey, LabelValue>, EngineError> {
    let mut labels = IndexMap::new();
    labels.insert(
        label_key("io.susun.project")?,
        label_value(project.identity.name.as_str())?,
    );
    labels.insert(
        label_key("io.susun.project-instance")?,
        label_value(project.identity.working_set.as_str())?,
    );
    labels.insert(label_key("io.susun.managed")?, label_value("true")?);
    labels.insert(
        label_key("io.susun.model-version")?,
        label_value("contract")?,
    );
    Ok(labels)
}

/// Creates a unique suffix appropriate for runtime resource names.
pub fn unique_suffix(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{nanos}")
}

fn resource_name(value: impl Into<String>) -> Result<ResourceName, EngineError> {
    ResourceName::new(value)
        .map_err(|error| EngineError::api(susun_engine::EngineOperation::Snapshot, error))
}

fn label_key(value: &str) -> Result<LabelKey, EngineError> {
    LabelKey::new(value)
        .map_err(|error| EngineError::api(susun_engine::EngineOperation::Snapshot, error))
}

fn label_value(value: &str) -> Result<LabelValue, EngineError> {
    LabelValue::new(value)
        .map_err(|error| EngineError::api(susun_engine::EngineOperation::Snapshot, error))
}

fn contract_error<T>(detail: &'static str) -> Result<T, EngineError> {
    Err(EngineError::api(
        EngineOperation::Snapshot,
        std::io::Error::other(detail),
    ))
}
