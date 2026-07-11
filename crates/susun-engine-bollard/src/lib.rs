//! Bollard-backed Docker Engine adapter.

use std::{collections::HashMap, time::SystemTime};

use bollard::{
    Docker,
    auth::DockerCredentials,
    body_full,
    container::LogOutput,
    exec::{CreateExecOptions, StartExecOptions, StartExecResults},
    models::{
        ContainerCreateBody, EndpointSettings, EventActor, EventMessage, HealthConfig, HostConfig,
        Mount, MountType as DockerMountType, NetworkCreateRequest, NetworkingConfig, PortBinding,
        PortMap, RestartPolicy, RestartPolicyNameEnum, VolumeCreateRequest,
    },
    query_parameters::{
        CreateContainerOptionsBuilder, CreateImageOptionsBuilder, DataUsageOptions,
        DownloadFromContainerOptionsBuilder, EventsOptionsBuilder, InspectContainerOptionsBuilder,
        ListContainersOptionsBuilder, ListImagesOptionsBuilder, ListNetworksOptionsBuilder,
        ListVolumesOptionsBuilder, LogsOptionsBuilder, PruneBuildOptionsBuilder,
        PushImageOptionsBuilder, RemoveContainerOptionsBuilder, RemoveImageOptionsBuilder,
        RemoveVolumeOptionsBuilder, StopContainerOptionsBuilder, TagImageOptionsBuilder,
        UploadToContainerOptionsBuilder, WaitContainerOptions,
    },
};
use futures_util::StreamExt;
use indexmap::IndexMap;
use susun_engine::{
    ArtifactMutationSchemaVersion, BoxByteStream, BoxEngineFuture, BoxEventStream, BoxExecStream,
    BoxLogStream, CleanupPreview, CleanupPreviewSchemaVersion, CleanupScopePreview,
    ContainerEngine, ContainerId, ContainerRef, CopyFromContainerRequest, CopyToContainerRequest,
    CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest, EngineApiVersion,
    EngineArchitecture, EngineCapabilities, EngineConnectionError, EngineConnectionProfile,
    EngineContainerInventory, EngineContainerSummary, EngineEndpoint, EngineError, EngineEvent,
    EngineImageInventory, EngineImageRef, EngineImageSummary, EngineInformation,
    EngineInventorySchemaVersion, EngineOperatingSystem, EngineOperation, EngineProbe,
    EngineProgressOperation, EngineSnapshot, EngineVersion, EventsRequest, ExecRequest,
    HealthState, ImageId, ImagePushRequest, ImagePushResult, ImageRemoveRequest, ImageRemoveResult,
    ImageTagRequest, ImageTagResult, LabelKey, LabelValue, LogEvent, LogSource, LogsRequest,
    MountType as EngineMountType, NetworkId, NetworkRef, ObservedContainer, ObservedImage,
    ObservedImageRef, ObservedNetwork, ObservedVolume, PortRequest, ProgressSink, ProjectIdentity,
    PruneReport, PruneRequest, PruneScope, PublishedPortBinding, PullImageRequest,
    ReclaimEstimateKind, RedactedEndpoint, RegistryAuthMaterial, ReplicaIndex, ResourceIdentity,
    ResourceName, RuntimeDoctorReport, ServiceInstanceId, SnapshotCompleteness,
    StopContainerRequest, SupportLevel, VolumeId, VolumeRef, WaitContainerRequest,
    WaitContainerResult,
};
use susun_model::{
    Command, Healthcheck, NetworkAttachment, PublishedPort,
    port::{CanonicalPort, Protocol},
    volume::VolumeKind,
};
use susun_secret::redact_sensitive_text;

/// Bollard-backed Docker Engine adapter.
#[derive(Debug, Clone)]
pub struct BollardEngine {
    docker: Docker,
    endpoint: EngineEndpoint,
}

const CONNECT_TIMEOUT_SECS: u64 = 10;

impl BollardEngine {
    /// Builds a client using Bollard's own local-discovery defaults.
    /// Validates configuration only — no network I/O.
    pub fn connect_local() -> Result<Self, EngineConnectionError> {
        let docker = Docker::connect_with_defaults().map_err(|error| {
            EngineConnectionError::EndpointUnavailable {
                endpoint: RedactedEndpoint::new(&EngineEndpoint::Local),
                source: Box::new(error),
            }
        })?;
        Ok(Self {
            docker,
            endpoint: EngineEndpoint::Local,
        })
    }

    /// Builds a client for an explicit endpoint. Validates configuration
    /// (including loading/parsing referenced TLS files) — no network I/O.
    pub fn connect_to(endpoint: EngineEndpoint) -> Result<Self, EngineConnectionError> {
        let docker = match &endpoint {
            EngineEndpoint::Local => Docker::connect_with_defaults(),
            EngineEndpoint::UnixSocket(path) => Docker::connect_with_socket(
                &path.to_string_lossy(),
                CONNECT_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            ),
            EngineEndpoint::WindowsNamedPipe(pipe) => {
                #[cfg(windows)]
                {
                    connect_named_pipe(pipe)
                }
                #[cfg(not(windows))]
                {
                    let _ = pipe;
                    return Err(EngineConnectionError::UnsupportedEndpoint {
                        endpoint_kind: endpoint.kind(),
                        platform: susun_engine::Platform::current(),
                    });
                }
            }
            EngineEndpoint::Tcp(tcp) => {
                let addr = format!("{}:{}", tcp.host(), tcp.port());
                match tcp.tls() {
                    None => Docker::connect_with_http(
                        &addr,
                        CONNECT_TIMEOUT_SECS,
                        bollard::API_DEFAULT_VERSION,
                    ),
                    Some(tls) => {
                        let Some(identity) = tls.client_identity() else {
                            return Err(EngineConnectionError::TlsConfiguration {
                                detail: "mutual TLS requires a client certificate and key"
                                    .to_owned(),
                            });
                        };
                        let Some(ca_certificate) = tls.ca_certificate() else {
                            return Err(EngineConnectionError::TlsConfiguration {
                                detail: "TLS requires a CA certificate path".to_owned(),
                            });
                        };
                        if tls.server_name().is_some() {
                            return Err(EngineConnectionError::TlsConfiguration {
                                detail: "TLS server-name override is not supported by susun-engine-bollard"
                                    .to_owned(),
                            });
                        }
                        Docker::connect_with_ssl(
                            &addr,
                            identity.private_key(),
                            identity.certificate(),
                            ca_certificate,
                            CONNECT_TIMEOUT_SECS,
                            bollard::API_DEFAULT_VERSION,
                        )
                    }
                }
            }
        }
        .map_err(|error| EngineConnectionError::EndpointUnavailable {
            endpoint: RedactedEndpoint::new(&endpoint),
            source: Box::new(error),
        })?;
        Ok(Self { docker, endpoint })
    }

    /// Proves reachability and fetches/validates the engine's reported
    /// API/version information. The only network call among these three
    /// constructors/methods.
    pub async fn probe(&self) -> Result<EngineProbe, EngineConnectionError> {
        let version =
            self.docker
                .version()
                .await
                .map_err(|error| EngineConnectionError::ApiNegotiation {
                    source: Box::new(error),
                })?;
        Ok(EngineProbe {
            api_version: version.api_version.map(EngineApiVersion::new),
            engine_version: version.version.map(EngineVersion::new),
            operating_system: version.os.map(EngineOperatingSystem::new),
            architecture: version.arch.map(EngineArchitecture::new),
        })
    }

    /// Connects to a profile, probes it, and returns a redacted readiness report.
    pub async fn doctor_profile(profile: &EngineConnectionProfile) -> RuntimeDoctorReport {
        let profile_id = Some(profile.id.clone());
        let endpoint = profile.endpoint().clone();
        let engine = match Self::connect_to(endpoint.clone()) {
            Ok(engine) => engine,
            Err(error) => {
                return RuntimeDoctorReport::from_connection_error(profile_id, &endpoint, &error);
            }
        };
        match engine.probe().await {
            Ok(probe) => RuntimeDoctorReport::available(profile_id, &endpoint, probe),
            Err(error) => RuntimeDoctorReport::from_connection_error(profile_id, &endpoint, &error),
        }
    }

    /// Returns the redacted endpoint.
    pub fn endpoint(&self) -> &EngineEndpoint {
        &self.endpoint
    }
}

#[cfg(windows)]
fn connect_named_pipe(pipe: &str) -> Result<Docker, bollard::errors::Error> {
    Docker::connect_with_named_pipe(pipe, CONNECT_TIMEOUT_SECS, bollard::API_DEFAULT_VERSION)
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
                supports_container_inventory: SupportLevel::Supported,
                supports_image_inventory: SupportLevel::Supported,
                supports_engine_information: SupportLevel::Supported,
                supports_image_management: SupportLevel::Supported,
                supports_registry_pull: SupportLevel::Supported,
                supports_registry_push: SupportLevel::SupportedSubset,
                supports_registry_auth: SupportLevel::Supported,
                supports_build_cache: SupportLevel::Supported,
                supports_cleanup_preview: SupportLevel::SupportedSubset,
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

    fn container_inventory(&self) -> BoxEngineFuture<'_, EngineContainerInventory> {
        Box::pin(async move {
            let containers = self
                .docker
                .list_containers(Some(
                    ListContainersOptionsBuilder::default()
                        .all(true)
                        .size(true)
                        .build(),
                ))
                .await
                .map_err(|error| EngineError::api(EngineOperation::ContainerInventory, error))?;
            let mut summaries = containers
                .into_iter()
                .filter_map(container_inventory_summary)
                .collect::<Result<Vec<_>, _>>()?;
            summaries.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
            Ok(EngineContainerInventory {
                schema_version: EngineInventorySchemaVersion::CURRENT,
                observed_at_epoch_seconds: unix_timestamp_now(),
                containers: summaries,
            })
        })
    }

    fn container_details(&self, id: &ContainerId) -> BoxEngineFuture<'_, EngineContainerSummary> {
        let id = id.clone();
        Box::pin(async move {
            self.container_inventory()
                .await?
                .containers
                .into_iter()
                .find(|container| container.id == id)
                .ok_or(EngineError::NotFound {
                    resource: ResourceIdentity::Container(id),
                })
        })
    }

    fn image_inventory(&self) -> BoxEngineFuture<'_, EngineImageInventory> {
        Box::pin(async move {
            let images = self
                .docker
                .list_images(Some(ListImagesOptionsBuilder::default().all(true).build()))
                .await
                .map_err(|error| EngineError::api(EngineOperation::ImageInventory, error))?;
            let mut summaries = images
                .into_iter()
                .filter_map(image_inventory_summary)
                .collect::<Vec<_>>();
            summaries.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
            Ok(EngineImageInventory {
                schema_version: EngineInventorySchemaVersion::CURRENT,
                observed_at_epoch_seconds: unix_timestamp_now(),
                images: summaries,
            })
        })
    }

    fn image_details(&self, id: &ImageId) -> BoxEngineFuture<'_, EngineImageSummary> {
        let id = id.clone();
        Box::pin(async move {
            self.image_inventory()
                .await?
                .images
                .into_iter()
                .find(|image| image.id == id)
                .ok_or(EngineError::NotFound {
                    resource: ResourceIdentity::Image(id.to_string()),
                })
        })
    }

    fn engine_information(&self) -> BoxEngineFuture<'_, EngineInformation> {
        Box::pin(async move {
            let version = self
                .docker
                .version()
                .await
                .map_err(|error| EngineError::api(EngineOperation::EngineInformation, error))?;
            let information = self
                .docker
                .info()
                .await
                .map_err(|error| EngineError::api(EngineOperation::EngineInformation, error))?;
            Ok(EngineInformation {
                schema_version: EngineInventorySchemaVersion::CURRENT,
                engine_version: version.version.map(EngineVersion::new),
                operating_system: information.operating_system.map(EngineOperatingSystem::new),
                architecture: information.architecture.map(EngineArchitecture::new),
                storage_driver: information.driver,
                logical_cpus: non_negative(information.ncpu),
                memory_bytes: non_negative(information.mem_total),
                container_count: non_negative(information.containers),
                running_container_count: non_negative(information.containers_running),
                image_count: non_negative(information.images),
            })
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
                        operation: EngineProgressOperation::PullImage,
                        stage: "pull".to_owned(),
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
                        message: event.status.as_deref().map(redact_sensitive_text),
                    })
                    .await;
            }
            Ok(EngineImageRef { reference: image })
        })
    }

    fn remove_image(&self, request: ImageRemoveRequest) -> BoxEngineFuture<'_, ImageRemoveResult> {
        Box::pin(async move {
            let responses = self
                .docker
                .remove_image(
                    request.image().as_str(),
                    Some(
                        RemoveImageOptionsBuilder::default()
                            .force(request.force())
                            .noprune(!request.prune_children())
                            .build(),
                    ),
                    None,
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::RemoveImage, error))?;
            let mut deleted = responses
                .iter()
                .filter_map(|item| item.deleted.as_deref())
                .filter_map(|id| ImageId::new(id.to_owned()).ok())
                .collect::<Vec<_>>();
            deleted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            deleted.dedup();
            let mut untagged = responses
                .iter()
                .filter_map(|item| item.untagged.as_deref())
                .map(|reference| susun_model::ImageRef::new(reference.to_owned()))
                .collect::<Vec<_>>();
            untagged.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            untagged.dedup();
            Ok(ImageRemoveResult {
                schema_version: ArtifactMutationSchemaVersion::CURRENT,
                deleted,
                untagged,
            })
        })
    }

    fn tag_image(&self, request: ImageTagRequest) -> BoxEngineFuture<'_, ImageTagResult> {
        Box::pin(async move {
            let (repository, tag) =
                split_repository_tag(request.target().as_str(), EngineOperation::TagImage)?;
            self.docker
                .tag_image(
                    request.source().as_str(),
                    Some(
                        TagImageOptionsBuilder::default()
                            .repo(repository)
                            .tag(tag)
                            .build(),
                    ),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::TagImage, error))?;
            Ok(ImageTagResult {
                schema_version: ArtifactMutationSchemaVersion::CURRENT,
                source: request.source().clone(),
                target: request.target().clone(),
            })
        })
    }

    fn push_image(
        &self,
        request: ImagePushRequest,
        progress: ProgressSink,
    ) -> BoxEngineFuture<'_, ImagePushResult> {
        Box::pin(async move {
            let (repository, tag) =
                split_repository_tag(request.image().as_str(), EngineOperation::PushImage)?;
            let mut stream = self.docker.push_image(
                repository,
                Some(PushImageOptionsBuilder::default().tag(tag).build()),
                None,
            );
            while let Some(item) = stream.next().await {
                let event =
                    item.map_err(|error| EngineError::api(EngineOperation::PushImage, error))?;
                progress
                    .emit(susun_engine::ActionProgress {
                        operation: EngineProgressOperation::PushImage,
                        stage: "push".to_owned(),
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
                        message: event.status.as_deref().map(redact_sensitive_text),
                    })
                    .await;
            }
            Ok(ImagePushResult {
                schema_version: ArtifactMutationSchemaVersion::CURRENT,
                image: request.image().clone(),
                digest: None,
                credential_ref: None,
            })
        })
    }

    fn push_image_authenticated(
        &self,
        request: ImagePushRequest,
        auth: RegistryAuthMaterial,
        progress: ProgressSink,
    ) -> BoxEngineFuture<'_, ImagePushResult> {
        Box::pin(async move {
            let credential_ref = request.credential_ref().cloned();
            let credentials = DockerCredentials {
                username: auth.username().map(str::to_owned),
                password: auth.password().map(str::to_owned),
                serveraddress: auth.server_address().map(str::to_owned),
                identitytoken: auth.identity_token_value().map(str::to_owned),
                registrytoken: auth.registry_token_value().map(str::to_owned),
                ..Default::default()
            };
            let (repository, tag) =
                split_repository_tag(request.image().as_str(), EngineOperation::PushImage)?;
            let mut stream = self.docker.push_image(
                repository,
                Some(PushImageOptionsBuilder::default().tag(tag).build()),
                Some(credentials),
            );
            while let Some(item) = stream.next().await {
                let event = item.map_err(|_| EngineError::Authentication {
                    registry: "<registry>".to_owned(),
                })?;
                if event.error_detail.is_some() {
                    return Err(EngineError::Authentication {
                        registry: "<registry>".to_owned(),
                    });
                }
                progress
                    .emit(susun_engine::ActionProgress {
                        operation: EngineProgressOperation::PushImage,
                        stage: "push".to_owned(),
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
                        message: event.status.as_deref().map(redact_sensitive_text),
                    })
                    .await;
            }
            Ok(ImagePushResult {
                schema_version: ArtifactMutationSchemaVersion::CURRENT,
                image: request.image().clone(),
                digest: None,
                credential_ref,
            })
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

    fn events(&self, request: EventsRequest) -> BoxEngineFuture<'_, BoxEventStream> {
        Box::pin(async move {
            let filters = label_filters(&request.project);
            let stream = self
                .docker
                .events(Some(
                    EventsOptionsBuilder::default().filters(&filters).build(),
                ))
                .map(|item| match item {
                    Ok(event) => Ok(engine_event(event)),
                    Err(error) => Err(EngineError::api(EngineOperation::Events, error)),
                });
            Ok(Box::pin(stream) as BoxEventStream)
        })
    }

    fn exec(&self, request: ExecRequest) -> BoxEngineFuture<'_, BoxExecStream> {
        Box::pin(async move {
            let created = self
                .docker
                .create_exec(
                    request.container.id.as_str(),
                    CreateExecOptions {
                        attach_stdin: Some(request.stdin),
                        attach_stdout: Some(true),
                        attach_stderr: Some(true),
                        tty: Some(request.tty),
                        cmd: Some(request.command),
                        user: request.user,
                        working_dir: request.working_dir,
                        ..Default::default()
                    },
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::Exec, error))?;
            match self
                .docker
                .start_exec(
                    &created.id,
                    Some(StartExecOptions {
                        tty: request.tty,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::Exec, error))?
            {
                StartExecResults::Attached { output, .. } => {
                    let stream = output.map(|item| match item {
                        Ok(output) => Ok(log_event(output)),
                        Err(error) => Err(EngineError::api(EngineOperation::Exec, error)),
                    });
                    Ok(Box::pin(stream) as BoxExecStream)
                }
                StartExecResults::Detached => Err(EngineError::Unsupported {
                    capability: "detached exec output",
                }),
            }
        })
    }

    fn copy_from_container(
        &self,
        request: CopyFromContainerRequest,
    ) -> BoxEngineFuture<'_, BoxByteStream> {
        Box::pin(async move {
            let stream = self
                .docker
                .download_from_container(
                    request.container.id.as_str(),
                    Some(
                        DownloadFromContainerOptionsBuilder::default()
                            .path(&request.path)
                            .build(),
                    ),
                )
                .map(|item| match item {
                    Ok(bytes) => Ok(bytes.to_vec()),
                    Err(error) => Err(EngineError::api(EngineOperation::Copy, error)),
                });
            Ok(Box::pin(stream) as BoxByteStream)
        })
    }

    fn copy_to_container(&self, request: CopyToContainerRequest) -> BoxEngineFuture<'_, ()> {
        Box::pin(async move {
            self.docker
                .upload_to_container(
                    request.container.id.as_str(),
                    Some(
                        UploadToContainerOptionsBuilder::default()
                            .path(&request.path)
                            .no_overwrite_dir_non_dir("true")
                            .build(),
                    ),
                    body_full(request.archive.into()),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::Copy, error))
        })
    }

    fn port(&self, request: PortRequest) -> BoxEngineFuture<'_, Vec<PublishedPortBinding>> {
        Box::pin(async move {
            let inspected = self
                .docker
                .inspect_container(
                    request.container.id.as_str(),
                    Some(
                        InspectContainerOptionsBuilder::default()
                            .size(false)
                            .build(),
                    ),
                )
                .await
                .map_err(|error| EngineError::api(EngineOperation::Port, error))?;
            let ports = inspected
                .network_settings
                .and_then(|settings| settings.ports)
                .unwrap_or_default();
            let mut bindings = Vec::new();
            for (key, values) in ports {
                let Some((private_port, protocol)) = parse_port_key(&key) else {
                    continue;
                };
                if request
                    .private_port
                    .is_some_and(|requested| requested != private_port)
                {
                    continue;
                }
                if request
                    .protocol
                    .as_ref()
                    .is_some_and(|requested| !requested.eq_ignore_ascii_case(protocol))
                {
                    continue;
                }
                for value in values.unwrap_or_default() {
                    let Some(host_port) = value.host_port else {
                        continue;
                    };
                    bindings.push(PublishedPortBinding {
                        private_port,
                        protocol: protocol.to_owned(),
                        host_ip: value.host_ip,
                        host_port,
                    });
                }
            }
            bindings.sort_by(|left, right| {
                (
                    left.private_port,
                    left.protocol.as_str(),
                    left.host_ip.as_deref(),
                    left.host_port.as_str(),
                )
                    .cmp(&(
                        right.private_port,
                        right.protocol.as_str(),
                        right.host_ip.as_deref(),
                        right.host_port.as_str(),
                    ))
            });
            Ok(bindings)
        })
    }

    fn cleanup_preview(&self, request: PruneRequest) -> BoxEngineFuture<'_, CleanupPreview> {
        Box::pin(async move {
            let usage = self
                .docker
                .df(None::<DataUsageOptions>)
                .await
                .map_err(|error| EngineError::api(EngineOperation::Prune, error))?;
            let scopes = request
                .scopes
                .iter()
                .copied()
                .map(|scope| match scope {
                    PruneScope::Containers => usage.container_usage.as_ref().map_or_else(
                        || unavailable_cleanup_scope(scope),
                        |value| {
                            aggregate_cleanup_scope(
                                scope,
                                value.total_count,
                                value.active_count,
                                value.reclaimable,
                                ReclaimEstimateKind::Exact,
                            )
                        },
                    ),
                    PruneScope::Volumes => usage.volume_usage.as_ref().map_or_else(
                        || unavailable_cleanup_scope(scope),
                        |value| {
                            aggregate_cleanup_scope(
                                scope,
                                value.total_count,
                                value.active_count,
                                value.reclaimable,
                                ReclaimEstimateKind::Exact,
                            )
                        },
                    ),
                    PruneScope::Images => usage.image_usage.as_ref().map_or_else(
                        || unavailable_cleanup_scope(scope),
                        |value| {
                            aggregate_cleanup_scope(
                                scope,
                                value.total_count,
                                value.active_count,
                                if request.all_images {
                                    value.reclaimable
                                } else {
                                    None
                                },
                                if request.all_images {
                                    ReclaimEstimateKind::Exact
                                } else {
                                    ReclaimEstimateKind::Unavailable
                                },
                            )
                        },
                    ),
                    PruneScope::BuildCache => usage.build_cache_usage.as_ref().map_or_else(
                        || unavailable_cleanup_scope(scope),
                        |value| {
                            aggregate_cleanup_scope(
                                scope,
                                value.total_count,
                                value.active_count,
                                value.reclaimable,
                                ReclaimEstimateKind::Exact,
                            )
                        },
                    ),
                    PruneScope::Networks => unavailable_cleanup_scope(scope),
                })
                .collect();
            Ok(CleanupPreview {
                schema_version: CleanupPreviewSchemaVersion::CURRENT,
                observed_at_epoch_seconds: unix_timestamp_now(),
                request,
                scopes,
            })
        })
    }

    fn prune(&self, request: PruneRequest) -> BoxEngineFuture<'_, PruneReport> {
        Box::pin(async move {
            let mut report = PruneReport::default();

            for scope in &request.scopes {
                match scope {
                    PruneScope::Containers => {
                        let response = self
                            .docker
                            .prune_containers(None)
                            .await
                            .map_err(|error| EngineError::api(EngineOperation::Prune, error))?;
                        report.containers_removed.extend(
                            response
                                .containers_deleted
                                .unwrap_or_default()
                                .into_iter()
                                .filter_map(|id| ContainerId::new(id).ok()),
                        );
                        report.space_reclaimed_bytes +=
                            u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0);
                    }
                    PruneScope::Networks => {
                        let response = self
                            .docker
                            .prune_networks(None)
                            .await
                            .map_err(|error| EngineError::api(EngineOperation::Prune, error))?;
                        report.networks_removed.extend(
                            response
                                .networks_deleted
                                .unwrap_or_default()
                                .into_iter()
                                .filter_map(|id| NetworkId::new(id).ok()),
                        );
                    }
                    PruneScope::Volumes => {
                        let response = self
                            .docker
                            .prune_volumes(None::<bollard::query_parameters::PruneVolumesOptions>)
                            .await
                            .map_err(|error| EngineError::api(EngineOperation::Prune, error))?;
                        report.volumes_removed.extend(
                            response
                                .volumes_deleted
                                .unwrap_or_default()
                                .into_iter()
                                .filter_map(|id| VolumeId::new(id).ok()),
                        );
                        report.space_reclaimed_bytes +=
                            u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0);
                    }
                    PruneScope::Images => {
                        let filter_options: Option<bollard::query_parameters::PruneImagesOptions> =
                            if request.all_images {
                                let mut filters = HashMap::new();
                                filters.insert("dangling".to_owned(), vec!["false".to_owned()]);
                                Some(
                                    bollard::query_parameters::PruneImagesOptionsBuilder::default()
                                        .filters(&filters)
                                        .build(),
                                )
                            } else {
                                None
                            };
                        let response = self
                            .docker
                            .prune_images(filter_options)
                            .await
                            .map_err(|error| EngineError::api(EngineOperation::Prune, error))?;
                        report.images_removed.extend(
                            response
                                .images_deleted
                                .unwrap_or_default()
                                .into_iter()
                                .filter_map(|item| item.deleted.or(item.untagged))
                                .filter_map(|id| ImageId::new(id).ok()),
                        );
                        report.space_reclaimed_bytes +=
                            u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0);
                    }
                    PruneScope::BuildCache => {
                        let response = self
                            .docker
                            .prune_build(Some(PruneBuildOptionsBuilder::default().build()))
                            .await
                            .map_err(|error| EngineError::api(EngineOperation::Prune, error))?;
                        report
                            .build_cache_records_removed
                            .extend(response.caches_deleted.unwrap_or_default());
                        report.space_reclaimed_bytes +=
                            u64::try_from(response.space_reclaimed.unwrap_or(0)).unwrap_or(0);
                    }
                }
            }

            Ok(report)
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

fn container_inventory_summary(
    container: bollard::models::ContainerSummary,
) -> Option<Result<EngineContainerSummary, EngineError>> {
    let id = container.id.and_then(new_container_id)?;
    let labels = map_labels(container.labels.unwrap_or_default());
    let name = container
        .names
        .and_then(|names| names.first().cloned())
        .unwrap_or_else(|| id.as_str().to_owned())
        .trim_start_matches('/')
        .to_owned();
    let name = match new_resource_name_for(name, EngineOperation::ContainerInventory) {
        Ok(name) => name,
        Err(error) => return Some(Err(error)),
    };
    let mut label_keys = labels.keys().cloned().collect::<Vec<_>>();
    label_keys.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    Some(Ok(EngineContainerSummary {
        id,
        name,
        state: container_state(container.state.map(|state| state.to_string())),
        health: container_health(container.status.as_deref()),
        image: container
            .image_id
            .and_then(new_image_id)
            .map(ObservedImageRef::Id)
            .or_else(|| {
                container
                    .image
                    .map(susun_model::ImageRef::new)
                    .map(ObservedImageRef::Reference)
            })
            .unwrap_or(ObservedImageRef::Unknown),
        project_identity: label_value(&labels, "io.susun.project-instance")
            .and_then(|value| susun_engine::ProjectInstanceId::new(value.to_owned()).ok()),
        label_keys,
        created_at_epoch_seconds: non_negative(container.created),
        writable_size_bytes: non_negative(container.size_rw),
        root_filesystem_size_bytes: non_negative(container.size_root_fs),
    }))
}

fn image_inventory_summary(image: bollard::models::ImageSummary) -> Option<EngineImageSummary> {
    let id = new_image_id(image.id)?;
    let mut references = image
        .repo_tags
        .into_iter()
        .map(susun_model::ImageRef::new)
        .collect::<Vec<_>>();
    references.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut digests = image.repo_digests;
    digests.sort();
    let mut label_keys = image
        .labels
        .into_keys()
        .filter_map(|key| LabelKey::new(key).ok())
        .collect::<Vec<_>>();
    label_keys.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    Some(EngineImageSummary {
        id,
        references,
        digests,
        label_keys,
        created_at_epoch_seconds: non_negative(Some(image.created)),
        size_bytes: non_negative(Some(image.size)),
        shared_size_bytes: non_negative(Some(image.shared_size)),
        container_count: non_negative(Some(image.containers)),
    })
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn aggregate_cleanup_scope(
    scope: PruneScope,
    total_count: Option<i64>,
    active_count: Option<i64>,
    reclaimable: Option<i64>,
    estimate_kind: ReclaimEstimateKind,
) -> CleanupScopePreview {
    let candidate_count = total_count
        .zip(active_count)
        .and_then(|(total, active)| u64::try_from(total.saturating_sub(active)).ok());
    CleanupScopePreview {
        scope,
        support: SupportLevel::SupportedSubset,
        candidate_count,
        reclaimable_bytes: non_negative(reclaimable),
        estimate_kind,
    }
}

fn unavailable_cleanup_scope(scope: PruneScope) -> CleanupScopePreview {
    CleanupScopePreview {
        scope,
        support: SupportLevel::Unsupported,
        candidate_count: None,
        reclaimable_bytes: None,
        estimate_kind: ReclaimEstimateKind::Unavailable,
    }
}

fn non_negative(value: Option<i64>) -> Option<u64> {
    value.and_then(|value| u64::try_from(value).ok())
}

fn split_repository_tag(
    reference: &str,
    operation: EngineOperation,
) -> Result<(&str, &str), EngineError> {
    if reference.contains('@') {
        return Err(EngineError::InvalidRequest {
            operation,
            detail: "digest references cannot be used as tag targets",
        });
    }
    let slash = reference.rfind('/');
    let colon = reference.rfind(':');
    match colon.filter(|colon| slash.is_none_or(|slash| *colon > slash)) {
        Some(colon) if colon > 0 && colon + 1 < reference.len() => {
            Ok((&reference[..colon], &reference[colon + 1..]))
        }
        _ if !reference.is_empty() => Ok((reference, "latest")),
        _ => Err(EngineError::InvalidRequest {
            operation,
            detail: "image reference must not be empty",
        }),
    }
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
    new_resource_name_for(value, EngineOperation::Snapshot)
}

fn new_resource_name_for(
    value: String,
    operation: EngineOperation,
) -> Result<ResourceName, EngineError> {
    ResourceName::new(value).map_err(|error| EngineError::Api {
        operation,
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
        Some(status) if status.contains("unhealthy") => HealthState::Unhealthy,
        Some(status) if status.contains("healthy") => HealthState::Healthy,
        _ => HealthState::Unknown,
    }
}

fn container_health(value: Option<&str>) -> Option<HealthState> {
    value
        .filter(|status| status.contains("health:"))
        .map(|status| health_state(Some(status)))
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

fn engine_event(event: EventMessage) -> EngineEvent {
    let EventMessage {
        typ,
        action,
        actor,
        time,
        time_nano,
        ..
    } = event;
    let EventActor { id, attributes } = actor.unwrap_or_default();
    EngineEvent {
        kind: typ
            .map(|kind| kind.as_ref().to_owned())
            .unwrap_or_else(|| "unknown".to_owned()),
        action: action.unwrap_or_else(|| "unknown".to_owned()),
        resource_id: id,
        attributes: safe_event_attributes(attributes.unwrap_or_default()),
        time,
        time_nano,
    }
}

fn safe_event_attributes(attributes: HashMap<String, String>) -> IndexMap<String, String> {
    attributes
        .into_iter()
        .filter(|(key, _)| {
            key.starts_with("io.susun.")
                || matches!(
                    key.as_str(),
                    "container" | "exitCode" | "health_status" | "image" | "name" | "signal"
                )
        })
        .collect()
}

fn parse_port_key(value: &str) -> Option<(u16, &str)> {
    let (port, protocol) = value.split_once('/')?;
    Some((port.parse().ok()?, protocol))
}

#[cfg(test)]
mod inventory_tests {
    use super::*;

    #[test]
    fn container_inventory_excludes_label_values_and_keeps_opaque_ownership()
    -> Result<(), Box<dyn std::error::Error>> {
        let container = bollard::models::ContainerSummary {
            id: Some("container-id".to_owned()),
            names: Some(vec!["/example".to_owned()]),
            labels: Some(HashMap::from([
                (
                    "io.susun.project-instance".to_owned(),
                    "opaque-project".to_owned(),
                ),
                (
                    "secret.example/token".to_owned(),
                    "do-not-expose".to_owned(),
                ),
            ])),
            ..Default::default()
        };

        let summary = container_inventory_summary(container)
            .ok_or_else(|| std::io::Error::other("missing inventory summary"))??;

        assert_eq!(
            summary.project_identity.as_ref().map(|id| id.as_str()),
            Some("opaque-project")
        );
        assert_eq!(
            summary
                .label_keys
                .iter()
                .map(LabelKey::as_str)
                .collect::<Vec<_>>(),
            vec!["io.susun.project-instance", "secret.example/token"]
        );
        Ok(())
    }

    #[test]
    fn artifact_reference_parser_handles_registry_ports_and_default_tags() -> Result<(), EngineError>
    {
        assert_eq!(
            split_repository_tag("localhost:5000/team/app:v2", EngineOperation::TagImage)?,
            ("localhost:5000/team/app", "v2")
        );
        assert_eq!(
            split_repository_tag("team/app", EngineOperation::PushImage)?,
            ("team/app", "latest")
        );
        Ok(())
    }

    #[test]
    fn artifact_reference_parser_rejects_digest_targets() {
        let result = split_repository_tag("team/app@sha256:0123", EngineOperation::TagImage);
        assert!(matches!(
            result,
            Err(EngineError::InvalidRequest {
                operation: EngineOperation::TagImage,
                ..
            })
        ));
    }

    #[test]
    fn cleanup_aggregate_reports_only_inactive_candidates() {
        let preview = aggregate_cleanup_scope(
            PruneScope::Containers,
            Some(7),
            Some(3),
            Some(4096),
            ReclaimEstimateKind::Exact,
        );
        assert_eq!(preview.candidate_count, Some(4));
        assert_eq!(preview.reclaimable_bytes, Some(4096));
        assert_eq!(preview.estimate_kind, ReclaimEstimateKind::Exact);
    }

    #[test]
    fn unsupported_cleanup_scope_never_invents_an_estimate() {
        let preview = unavailable_cleanup_scope(PruneScope::Networks);
        assert_eq!(preview.support, SupportLevel::Unsupported);
        assert_eq!(preview.candidate_count, None);
        assert_eq!(preview.reclaimable_bytes, None);
        assert_eq!(preview.estimate_kind, ReclaimEstimateKind::Unavailable);
    }

    #[test]
    fn docker_desktop_and_podman_usage_fixtures_preserve_missing_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let docker: bollard::models::SystemDataUsageResponse = serde_json::from_str(include_str!(
            "../tests/fixtures/docker-desktop-data-usage.json"
        ))?;
        let podman: bollard::models::SystemDataUsageResponse =
            serde_json::from_str(include_str!("../tests/fixtures/podman-data-usage.json"))?;

        assert_eq!(
            docker.build_cache_usage.and_then(|usage| usage.reclaimable),
            Some(65536)
        );
        assert!(podman.build_cache_usage.is_none());
        assert!(podman.volume_usage.is_none());
        assert_eq!(podman.image_usage.and_then(|usage| usage.reclaimable), None);
        Ok(())
    }
}
