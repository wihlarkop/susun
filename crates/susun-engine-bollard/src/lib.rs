//! Bollard-backed Docker Engine adapter.

use std::{collections::HashMap, time::SystemTime};

use bollard::{
    Docker,
    container::LogOutput,
    models::{ContainerCreateBody, NetworkCreateRequest, VolumeCreateRequest},
    query_parameters::{
        CreateContainerOptionsBuilder, CreateImageOptionsBuilder, ListContainersOptionsBuilder,
        ListImagesOptionsBuilder, ListNetworksOptionsBuilder, ListVolumesOptionsBuilder,
        LogsOptionsBuilder, RemoveContainerOptionsBuilder, RemoveVolumeOptionsBuilder,
        StopContainerOptionsBuilder,
    },
};
use futures_util::StreamExt;
use indexmap::IndexMap;
use susun_engine::{
    BoxEngineFuture, BoxLogStream, ContainerEngine, ContainerId, ContainerRef,
    CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest, EngineApiVersion,
    EngineCapabilities, EngineEndpoint, EngineError, EngineImageRef, EngineOperation,
    EngineSnapshot, HealthState, LabelKey, LabelValue, LogEvent, LogSource, LogsRequest, MountType,
    NetworkId, NetworkRef, ObservedContainer, ObservedImage, ObservedImageRef, ObservedNetwork,
    ObservedVolume, ProgressSink, ProjectIdentity, PullImageRequest, ReplicaIndex,
    ResourceIdentity, ResourceName, ServiceInstanceId, SnapshotCompleteness, StopContainerRequest,
    SupportLevel, VolumeId, VolumeRef,
};

/// Bollard-backed Docker Engine adapter.
#[derive(Debug, Clone)]
pub struct BollardEngine {
    docker: Docker,
    endpoint: EngineEndpoint,
}

impl BollardEngine {
    /// Connects to the local Docker Engine using Bollard defaults.
    pub fn connect_local() -> Result<Self, EngineError> {
        let docker = Docker::connect_with_defaults().map_err(|error| EngineError::Api {
            operation: EngineOperation::Capabilities,
            source: Box::new(error),
        })?;
        Ok(Self {
            docker,
            endpoint: EngineEndpoint::Local,
        })
    }

    /// Returns the redacted endpoint.
    pub fn endpoint(&self) -> &EngineEndpoint {
        &self.endpoint
    }
}

impl ContainerEngine for BollardEngine {
    fn capabilities(&self) -> BoxEngineFuture<'_, EngineCapabilities> {
        Box::pin(async move {
            let version = self
                .docker
                .version()
                .await
                .map_err(|error| EngineError::api(EngineOperation::Capabilities, error))?;
            Ok(EngineCapabilities {
                api_version: version.api_version.map(EngineApiVersion::new),
                supports_health: SupportLevel::SupportedSubset,
                supports_named_volumes: SupportLevel::Supported,
                supports_network_aliases: SupportLevel::SupportedSubset,
                supports_mount_types: [MountType::Volume, MountType::Bind, MountType::Anonymous]
                    .into_iter()
                    .collect(),
                supports_log_follow: SupportLevel::Supported,
                supports_build: SupportLevel::Unsupported,
                max_container_name_length: Some(255),
            })
        })
    }

    fn snapshot(&self, project: &ProjectIdentity) -> BoxEngineFuture<'_, EngineSnapshot> {
        let project = project.clone();
        Box::pin(async move {
            let mut snapshot = EngineSnapshot::empty(SystemTime::now());
            let filters = label_filters(&project);

            let containers = self
                .docker
                .list_containers(Some(
                    ListContainersOptionsBuilder::default()
                        .all(true)
                        .filters(&filters)
                        .build(),
                ))
                .await
                .map_err(|error| EngineError::api(EngineOperation::Snapshot, error))?;
            for container in containers {
                if let Some(id) = container.id.and_then(new_container_id) {
                    let labels = map_labels(container.labels.unwrap_or_default());
                    let name = container
                        .names
                        .and_then(|names| names.first().cloned())
                        .unwrap_or_else(|| id.as_str().to_owned())
                        .trim_start_matches('/')
                        .to_owned();
                    let name = new_resource_name(name)?;
                    let service_identity =
                        label_value(&labels, "io.susun.service").map(|service| {
                            ServiceInstanceId::new(
                                project.working_set.clone(),
                                susun_model::ServiceName::new(service.to_owned()),
                                label_value(&labels, "io.susun.replica")
                                    .and_then(|replica| replica.parse::<u32>().ok())
                                    .map(|ordinal| ReplicaIndex::new(ordinal.saturating_sub(1)))
                                    .unwrap_or_default(),
                            )
                        });
                    let observed = ObservedContainer {
                        id: id.clone(),
                        name,
                        state: container_state(
                            container.state.as_ref().map(|state| state.to_string()),
                        ),
                        health: Some(health_state(container.status.as_deref())),
                        image: container
                            .image
                            .map(susun_model::ImageRef::new)
                            .map(ObservedImageRef::Reference)
                            .unwrap_or(ObservedImageRef::Unknown),
                        project_identity: Some(project.working_set.clone()),
                        service_identity,
                        configuration_fingerprint: None,
                        labels,
                        completeness: SnapshotCompleteness::Complete,
                    };
                    snapshot.containers.insert(id, observed);
                }
            }

            let networks = self
                .docker
                .list_networks(Some(
                    ListNetworksOptionsBuilder::default()
                        .filters(&filters)
                        .build(),
                ))
                .await
                .map_err(|error| EngineError::api(EngineOperation::Snapshot, error))?;
            for network in networks {
                if let (Some(id), Some(name)) =
                    (network.id.map(new_network_id).transpose()?, network.name)
                {
                    let labels = map_labels(network.labels.unwrap_or_default());
                    let network_identity =
                        label_value(&labels, "io.susun.network").map(|network| {
                            susun_engine::NetworkIdentity::new(
                                project.working_set.clone(),
                                susun_model::NetworkName::new(network.to_owned()),
                            )
                        });
                    snapshot.networks.insert(
                        id.clone(),
                        ObservedNetwork {
                            id,
                            name: new_resource_name(name)?,
                            labels,
                            project_identity: Some(project.working_set.clone()),
                            network_identity,
                            completeness: SnapshotCompleteness::Complete,
                        },
                    );
                }
            }

            let volumes = self
                .docker
                .list_volumes(Some(
                    ListVolumesOptionsBuilder::default()
                        .filters(&filters)
                        .build(),
                ))
                .await
                .map_err(|error| EngineError::api(EngineOperation::Snapshot, error))?;
            if let Some(volumes) = volumes.volumes {
                for volume in volumes {
                    let id = new_volume_id(volume.name.clone())?;
                    let labels = map_labels(volume.labels);
                    let volume_identity = label_value(&labels, "io.susun.volume").map(|volume| {
                        susun_engine::VolumeIdentity::new(
                            project.working_set.clone(),
                            susun_model::VolumeName::new(volume.to_owned()),
                        )
                    });
                    snapshot.volumes.insert(
                        id.clone(),
                        ObservedVolume {
                            id,
                            name: new_resource_name(volume.name)?,
                            labels,
                            project_identity: Some(project.working_set.clone()),
                            volume_identity,
                            completeness: SnapshotCompleteness::Complete,
                        },
                    );
                }
            }

            let images = self
                .docker
                .list_images(Some(ListImagesOptionsBuilder::default().all(true).build()))
                .await
                .map_err(|error| EngineError::api(EngineOperation::Snapshot, error))?;
            for image in images {
                if let Some(id) = new_image_id(image.id) {
                    snapshot.images.insert(
                        id.clone(),
                        ObservedImage {
                            id,
                            references: image
                                .repo_tags
                                .into_iter()
                                .map(susun_model::ImageRef::new)
                                .collect(),
                            labels: map_labels(image.labels),
                            completeness: SnapshotCompleteness::Complete,
                        },
                    );
                }
            }

            Ok(snapshot)
        })
    }

    fn pull_image(
        &self,
        request: PullImageRequest,
        progress: ProgressSink,
    ) -> BoxEngineFuture<'_, EngineImageRef> {
        Box::pin(async move {
            let image = request.image.as_str().to_owned();
            let mut stream = self.docker.create_image(
                Some(
                    CreateImageOptionsBuilder::default()
                        .from_image(&image)
                        .build(),
                ),
                None,
                None,
            );
            while let Some(item) = stream.next().await {
                let event =
                    item.map_err(|error| EngineError::api(EngineOperation::PullImage, error))?;
                progress
                    .emit(susun_engine::ActionProgress {
                        stage: event.status.unwrap_or_else(|| "pull".to_owned()),
                        current: event
                            .progress_detail
                            .as_ref()
                            .and_then(|detail| detail.current)
                            .and_then(|value| u64::try_from(value).ok()),
                        total: event
                            .progress_detail
                            .as_ref()
                            .and_then(|detail| detail.total)
                            .and_then(|value| u64::try_from(value).ok()),
                        message: None,
                    })
                    .await;
            }
            Ok(EngineImageRef { reference: image })
        })
    }

    fn create_network(&self, request: CreateNetworkRequest) -> BoxEngineFuture<'_, NetworkRef> {
        Box::pin(async move {
            let response = self
                .docker
                .create_network(NetworkCreateRequest {
                    name: request.name.as_str().to_owned(),
                    labels: Some(labels_to_hashmap(request.labels)),
                    ..Default::default()
                })
                .await
                .map_err(|error| EngineError::api(EngineOperation::CreateNetwork, error))?;
            Ok(NetworkRef {
                id: new_network_id(response.id)?,
            })
        })
    }

    fn remove_network(&self, id: NetworkRef) -> BoxEngineFuture<'_, ()> {
        Box::pin(async move {
            self.docker
                .remove_network(id.id.as_str())
                .await
                .map_err(|error| EngineError::api(EngineOperation::RemoveNetwork, error))
        })
    }

    fn create_volume(&self, request: CreateVolumeRequest) -> BoxEngineFuture<'_, VolumeRef> {
        Box::pin(async move {
            let response = self
                .docker
                .create_volume(VolumeCreateRequest {
                    name: Some(request.name.as_str().to_owned()),
                    labels: Some(labels_to_hashmap(request.labels)),
                    ..Default::default()
                })
                .await
                .map_err(|error| EngineError::api(EngineOperation::CreateVolume, error))?;
            Ok(VolumeRef {
                id: new_volume_id(response.name)?,
            })
        })
    }

    fn remove_volume(&self, id: VolumeRef) -> BoxEngineFuture<'_, ()> {
        Box::pin(async move {
            self.docker
                .remove_volume(
                    id.id.as_str(),
                    Some(RemoveVolumeOptionsBuilder::default().force(false).build()),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::RemoveVolume, error))
        })
    }

    fn create_container(
        &self,
        request: CreateContainerRequest,
    ) -> BoxEngineFuture<'_, ContainerRef> {
        Box::pin(async move {
            let response = self
                .docker
                .create_container(
                    Some(
                        CreateContainerOptionsBuilder::default()
                            .name(request.name.as_str())
                            .build(),
                    ),
                    ContainerCreateBody {
                        image: request.image.map(|image| image.as_str().to_owned()),
                        labels: Some(labels_to_hashmap(request.labels)),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::CreateContainer, error))?;
            Ok(ContainerRef {
                id: new_container_id(response.id).ok_or_else(|| EngineError::NotFound {
                    resource: ResourceIdentity::Name(request.name),
                })?,
            })
        })
    }

    fn start_container(&self, id: &ContainerRef) -> BoxEngineFuture<'_, ()> {
        let id = id.id.as_str().to_owned();
        Box::pin(async move {
            self.docker
                .start_container(&id, None)
                .await
                .map_err(|error| EngineError::api(EngineOperation::StartContainer, error))
        })
    }

    fn stop_container(&self, request: StopContainerRequest) -> BoxEngineFuture<'_, ()> {
        Box::pin(async move {
            let seconds = i32::try_from(request.timeout.as_secs()).unwrap_or(i32::MAX);
            self.docker
                .stop_container(
                    request.container.id.as_str(),
                    Some(StopContainerOptionsBuilder::default().t(seconds).build()),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::StopContainer, error))
        })
    }

    fn remove_container(
        &self,
        id: &ContainerRef,
        options: susun_engine::RemoveContainerOptions,
    ) -> BoxEngineFuture<'_, ()> {
        let id = id.id.as_str().to_owned();
        Box::pin(async move {
            self.docker
                .remove_container(
                    &id,
                    Some(
                        RemoveContainerOptionsBuilder::default()
                            .v(options.remove_anonymous_volumes)
                            .force(options.force)
                            .build(),
                    ),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::RemoveContainer, error))
        })
    }

    fn logs(&self, request: LogsRequest) -> BoxEngineFuture<'_, BoxLogStream> {
        Box::pin(async move {
            let tail = request.tail.map(|tail| tail.to_string());
            let tail_value = tail.unwrap_or_else(|| "all".to_owned());
            let stream = self
                .docker
                .logs(
                    request.container.id.as_str(),
                    Some(
                        LogsOptionsBuilder::default()
                            .stdout(true)
                            .stderr(true)
                            .follow(request.follow)
                            .timestamps(request.timestamps)
                            .tail(&tail_value)
                            .build(),
                    ),
                )
                .map(|item| match item {
                    Ok(output) => Ok(log_event(output)),
                    Err(error) => Err(EngineError::api(EngineOperation::Logs, error)),
                });
            Ok(Box::pin(stream) as BoxLogStream)
        })
    }
}

fn label_filters(project: &ProjectIdentity) -> HashMap<String, Vec<String>> {
    HashMap::from([(
        "label".to_owned(),
        vec![format!(
            "io.susun.project-instance={}",
            project.working_set.as_str()
        )],
    )])
}

fn map_labels(labels: HashMap<String, String>) -> IndexMap<LabelKey, LabelValue> {
    labels
        .into_iter()
        .filter_map(|(key, value)| Some((LabelKey::new(key).ok()?, LabelValue::new(value).ok()?)))
        .collect()
}

fn labels_to_hashmap(labels: IndexMap<LabelKey, LabelValue>) -> HashMap<String, String> {
    labels
        .into_iter()
        .map(|(key, value)| (key.as_str().to_owned(), value.as_str().to_owned()))
        .collect()
}

fn label_value<'a>(labels: &'a IndexMap<LabelKey, LabelValue>, key: &str) -> Option<&'a str> {
    labels
        .iter()
        .find_map(|(candidate, value)| (candidate.as_str() == key).then_some(value.as_str()))
}

fn new_resource_name(value: String) -> Result<ResourceName, EngineError> {
    ResourceName::new(value).map_err(|error| EngineError::Api {
        operation: EngineOperation::Snapshot,
        source: Box::new(error),
    })
}

fn new_container_id(value: String) -> Option<ContainerId> {
    ContainerId::new(value).ok()
}

fn new_network_id(value: String) -> Result<NetworkId, EngineError> {
    NetworkId::new(value).map_err(|error| EngineError::Api {
        operation: EngineOperation::Snapshot,
        source: Box::new(error),
    })
}

fn new_volume_id(value: String) -> Result<VolumeId, EngineError> {
    VolumeId::new(value).map_err(|error| EngineError::Api {
        operation: EngineOperation::Snapshot,
        source: Box::new(error),
    })
}

fn new_image_id(value: String) -> Option<susun_engine::ImageId> {
    susun_engine::ImageId::new(value).ok()
}

fn container_state(value: Option<String>) -> susun_engine::ContainerState {
    match value.as_deref() {
        Some("created") | Some("Created") => susun_engine::ContainerState::Created,
        Some("running") | Some("Running") => susun_engine::ContainerState::Running,
        Some("exited") | Some("Exited") => susun_engine::ContainerState::Exited,
        Some("paused") | Some("Paused") => susun_engine::ContainerState::Paused,
        Some("restarting") | Some("Restarting") => susun_engine::ContainerState::Restarting,
        _ => susun_engine::ContainerState::Unknown,
    }
}

fn health_state(value: Option<&str>) -> HealthState {
    match value {
        Some(status) if status.contains("healthy") => HealthState::Healthy,
        Some(status) if status.contains("unhealthy") => HealthState::Unhealthy,
        _ => HealthState::Unknown,
    }
}

fn log_event(output: LogOutput) -> LogEvent {
    match output {
        LogOutput::StdOut { message } => LogEvent {
            source: LogSource::Stdout,
            line: String::from_utf8_lossy(&message).into_owned(),
        },
        LogOutput::StdErr { message } => LogEvent {
            source: LogSource::Stderr,
            line: String::from_utf8_lossy(&message).into_owned(),
        },
        LogOutput::StdIn { message } | LogOutput::Console { message } => LogEvent {
            source: LogSource::Unknown,
            line: String::from_utf8_lossy(&message).into_owned(),
        },
    }
}
