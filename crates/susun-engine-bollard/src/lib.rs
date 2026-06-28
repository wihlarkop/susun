//! Bollard-backed Docker Engine adapter.

use std::{collections::HashMap, time::SystemTime};

use bollard::{
    Docker,
    container::LogOutput,
    models::{
        ContainerCreateBody, EndpointSettings, HealthConfig, HostConfig, Mount,
        MountType as DockerMountType, NetworkCreateRequest, NetworkingConfig, PortBinding, PortMap,
        RestartPolicy, RestartPolicyNameEnum, VolumeCreateRequest,
    },
    query_parameters::{
        CreateContainerOptionsBuilder, CreateImageOptionsBuilder, ListContainersOptionsBuilder,
        ListImagesOptionsBuilder, ListNetworksOptionsBuilder, ListVolumesOptionsBuilder,
        LogsOptionsBuilder, RemoveContainerOptionsBuilder, RemoveVolumeOptionsBuilder,
        StopContainerOptionsBuilder, WaitContainerOptions,
    },
};
use futures_util::StreamExt;
use indexmap::IndexMap;
use susun_engine::{
    BoxEngineFuture, BoxLogStream, ContainerEngine, ContainerId, ContainerRef,
    CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest, EngineApiVersion,
    EngineCapabilities, EngineEndpoint, EngineError, EngineImageRef, EngineOperation,
    EngineSnapshot, HealthState, LabelKey, LabelValue, LogEvent, LogSource, LogsRequest,
    MountType as EngineMountType, NetworkId, NetworkRef, ObservedContainer, ObservedImage,
    ObservedImageRef, ObservedNetwork, ObservedVolume, ProgressSink, ProjectIdentity,
    PullImageRequest, ReplicaIndex, ResourceIdentity, ResourceName, ServiceInstanceId,
    SnapshotCompleteness, StopContainerRequest, SupportLevel, VolumeId, VolumeRef,
    WaitContainerRequest, WaitContainerResult,
};
use susun_model::{
    Command, Healthcheck, NetworkAttachment, PublishedPort,
    port::{CanonicalPort, Protocol},
    volume::VolumeKind,
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
                supports_mount_types: [
                    EngineMountType::Volume,
                    EngineMountType::Bind,
                    EngineMountType::Anonymous,
                ]
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
            let host_config = host_config(&request);
            let networking_config = networking_config(&request.networks);
            let labels = merged_labels(request.container_labels, request.labels);
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
                        env: env_values(&request.environment),
                        cmd: request.command.as_ref().map(command_to_vec),
                        entrypoint: request.entrypoint.as_ref().map(command_to_vec),
                        exposed_ports: exposed_ports(&request.ports),
                        healthcheck: request.healthcheck.as_ref().map(health_config),
                        labels: Some(labels),
                        host_config: Some(host_config),
                        networking_config,
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

    fn wait_container(
        &self,
        request: WaitContainerRequest,
    ) -> BoxEngineFuture<'_, WaitContainerResult> {
        Box::pin(async move {
            let mut stream = self
                .docker
                .wait_container(request.container.id.as_str(), None::<WaitContainerOptions>);
            match stream.next().await {
                Some(Ok(result)) => Ok(WaitContainerResult {
                    exit_code: result.status_code,
                }),
                Some(Err(error)) => Err(EngineError::api(EngineOperation::Wait, error)),
                None => Err(EngineError::Unsupported {
                    capability: "container wait returned no status",
                }),
            }
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

fn merged_labels(
    container_labels: IndexMap<String, String>,
    ownership: IndexMap<LabelKey, LabelValue>,
) -> HashMap<String, String> {
    let mut labels = container_labels.into_iter().collect::<HashMap<_, _>>();
    labels.extend(labels_to_hashmap(ownership));
    labels
}

fn env_values(environment: &IndexMap<String, Option<String>>) -> Option<Vec<String>> {
    if environment.is_empty() {
        return None;
    }
    Some(
        environment
            .iter()
            .map(|(key, value)| match value {
                Some(value) => format!("{key}={value}"),
                None => key.clone(),
            })
            .collect(),
    )
}

fn command_to_vec(command: &Command) -> Vec<String> {
    match command {
        Command::Shell(value) => vec![value.clone()],
        Command::Exec(values) => values.clone(),
    }
}

fn exposed_ports(ports: &[CanonicalPort]) -> Option<Vec<String>> {
    if ports.is_empty() {
        return None;
    }
    Some(ports.iter().map(port_key).collect())
}

fn host_config(request: &CreateContainerRequest) -> HostConfig {
    HostConfig {
        port_bindings: port_bindings(&request.ports),
        mounts: mount_values(request),
        restart_policy: request.restart.as_deref().and_then(restart_policy),
        network_mode: request
            .networks
            .keys()
            .next()
            .map(|network| network.as_str().to_owned()),
        ..Default::default()
    }
}

fn port_bindings(ports: &[CanonicalPort]) -> Option<PortMap> {
    let mut map = HashMap::new();
    for port in ports {
        let Some(published) = port.published else {
            continue;
        };
        let bindings = match published {
            PublishedPort::Single(port_value) => vec![PortBinding {
                host_ip: port.host_ip.clone(),
                host_port: Some(port_value.to_string()),
            }],
            PublishedPort::Range { start, end } => (start..=end)
                .map(|port_value| PortBinding {
                    host_ip: port.host_ip.clone(),
                    host_port: Some(port_value.to_string()),
                })
                .collect(),
        };
        map.insert(port_key(port), Some(bindings));
    }
    (!map.is_empty()).then_some(map)
}

fn port_key(port: &CanonicalPort) -> String {
    let protocol = match port.protocol {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
        Protocol::Sctp => "sctp",
    };
    format!("{}/{}", port.target, protocol)
}

fn mount_values(request: &CreateContainerRequest) -> Option<Vec<Mount>> {
    if request.volumes.is_empty() && request.configs.is_empty() && request.secrets.is_empty() {
        return None;
    }
    let mut mounts = request
        .volumes
        .iter()
        .map(|volume| Mount {
            target: Some(volume.target.clone()),
            source: volume.source.clone(),
            typ: Some(match volume.kind {
                VolumeKind::Volume | VolumeKind::Anonymous => DockerMountType::VOLUME,
                VolumeKind::Bind => DockerMountType::BIND,
            }),
            read_only: Some(volume.read_only),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    mounts.extend(
        request
            .configs
            .iter()
            .chain(request.secrets.iter())
            .map(|mount| Mount {
                target: Some(mount.target.clone()),
                source: Some(mount.source.to_string_lossy().into_owned()),
                typ: Some(DockerMountType::BIND),
                read_only: Some(true),
                ..Default::default()
            }),
    );
    Some(mounts)
}

fn networking_config(
    networks: &IndexMap<ResourceName, NetworkAttachment>,
) -> Option<NetworkingConfig> {
    if networks.is_empty() {
        return None;
    }
    let endpoints_config = networks
        .iter()
        .map(|(name, attachment)| {
            (
                name.as_str().to_owned(),
                EndpointSettings {
                    aliases: (!attachment.aliases.is_empty()).then_some(attachment.aliases.clone()),
                    ..Default::default()
                },
            )
        })
        .collect();
    Some(NetworkingConfig {
        endpoints_config: Some(endpoints_config),
    })
}

fn health_config(healthcheck: &Healthcheck) -> HealthConfig {
    HealthConfig {
        test: health_test(healthcheck),
        interval: healthcheck.interval.as_deref().and_then(duration_to_nanos),
        timeout: healthcheck.timeout.as_deref().and_then(duration_to_nanos),
        start_period: healthcheck
            .start_period
            .as_deref()
            .and_then(duration_to_nanos),
        retries: healthcheck.retries.map(i64::from),
        ..Default::default()
    }
}

fn health_test(healthcheck: &Healthcheck) -> Option<Vec<String>> {
    if healthcheck.disable {
        return Some(vec!["NONE".to_owned()]);
    }
    healthcheck.test.as_ref().map(|command| match command {
        Command::Shell(value) => vec!["CMD-SHELL".to_owned(), value.clone()],
        Command::Exec(values) => std::iter::once("CMD".to_owned())
            .chain(values.iter().cloned())
            .collect(),
    })
}

fn duration_to_nanos(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    let (number, multiplier) = if let Some(number) = trimmed.strip_suffix("ms") {
        (number, 1_000_000_i64)
    } else if let Some(number) = trimmed.strip_suffix("us") {
        (number, 1_000_i64)
    } else if let Some(number) = trimmed.strip_suffix("ns") {
        (number, 1_i64)
    } else if let Some(number) = trimmed.strip_suffix('s') {
        (number, 1_000_000_000_i64)
    } else if let Some(number) = trimmed.strip_suffix('m') {
        (number, 60_000_000_000_i64)
    } else {
        (trimmed, 1_000_000_000_i64)
    };
    number
        .parse::<i64>()
        .ok()
        .and_then(|value| value.checked_mul(multiplier))
}

fn restart_policy(value: &str) -> Option<RestartPolicy> {
    let (name, maximum_retry_count) = value
        .split_once(':')
        .map(|(name, count)| (name, count.parse::<i64>().ok()))
        .unwrap_or((value, None));
    let name = match name {
        "" => RestartPolicyNameEnum::EMPTY,
        "no" => RestartPolicyNameEnum::NO,
        "always" => RestartPolicyNameEnum::ALWAYS,
        "unless-stopped" => RestartPolicyNameEnum::UNLESS_STOPPED,
        "on-failure" => RestartPolicyNameEnum::ON_FAILURE,
        _ => return None,
    };
    Some(RestartPolicy {
        name: Some(name),
        maximum_retry_count,
    })
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
