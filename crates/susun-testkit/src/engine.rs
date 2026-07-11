//! Fake neutral container engine for runtime failure tests.

use std::time::SystemTime;

use susun_engine::{
    ArtifactMutationSchemaVersion, BoxByteStream, BoxEngineFuture, BoxEventStream, BoxExecStream,
    BoxLogStream, ContainerEngine, ContainerId, ContainerRef, CopyFromContainerRequest,
    CopyToContainerRequest, CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest,
    EngineCapabilities, EngineContainerInventory, EngineContainerSummary, EngineError,
    EngineImageInventory, EngineImageRef, EngineImageSummary, EngineInformation,
    EngineInventorySchemaVersion, EngineOperation, EngineProgressOperation, EngineSnapshot,
    EventsRequest, ExecRequest, ImagePushRequest, ImagePushResult, ImageRemoveRequest,
    ImageRemoveResult, ImageTagRequest, ImageTagResult, LogsRequest, NetworkId, NetworkRef,
    PortRequest, ProgressSink, ProjectIdentity, PruneReport, PruneRequest, PublishedPortBinding,
    PullImageRequest, RemoveContainerOptions, ResourceIdentity, StopContainerRequest, VolumeId,
    VolumeRef, WaitContainerRequest, WaitContainerResult,
};

/// In-memory engine that can fail selected operations.
#[derive(Debug, Clone, Default)]
pub struct FakeContainerEngine {
    failures: Vec<EngineOperation>,
    snapshot: Option<EngineSnapshot>,
    container_inventory: Option<EngineContainerInventory>,
    image_inventory: Option<EngineImageInventory>,
    engine_information: Option<EngineInformation>,
}

impl FakeContainerEngine {
    /// Creates a fake engine with no failures and an empty snapshot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a fake engine that fails the selected operation.
    #[must_use]
    pub fn failing(operation: EngineOperation) -> Self {
        Self::new().fail(operation)
    }

    /// Adds a failing operation.
    #[must_use]
    pub fn fail(mut self, operation: EngineOperation) -> Self {
        if !self.failures.contains(&operation) {
            self.failures.push(operation);
        }
        self
    }

    /// Replaces the snapshot returned by this fake engine.
    #[must_use]
    pub fn with_snapshot(mut self, snapshot: EngineSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Replaces the engine-wide container inventory returned by this fake.
    #[must_use]
    pub fn with_container_inventory(mut self, inventory: EngineContainerInventory) -> Self {
        self.container_inventory = Some(inventory);
        self
    }

    /// Replaces the engine-wide image inventory returned by this fake.
    #[must_use]
    pub fn with_image_inventory(mut self, inventory: EngineImageInventory) -> Self {
        self.image_inventory = Some(inventory);
        self
    }

    /// Replaces the engine information returned by this fake.
    #[must_use]
    pub fn with_engine_information(mut self, information: EngineInformation) -> Self {
        self.engine_information = Some(information);
        self
    }

    fn should_fail(&self, operation: EngineOperation) -> bool {
        self.failures.contains(&operation)
    }
}

impl ContainerEngine for FakeContainerEngine {
    fn capabilities(&self) -> BoxEngineFuture<'_, EngineCapabilities> {
        let fail = self.should_fail(EngineOperation::Capabilities);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::Capabilities))
            } else {
                let mut capabilities = EngineCapabilities::permissive_local();
                capabilities.supports_container_inventory = susun_engine::SupportLevel::Supported;
                capabilities.supports_image_inventory = susun_engine::SupportLevel::Supported;
                capabilities.supports_engine_information = susun_engine::SupportLevel::Supported;
                capabilities.supports_image_management = susun_engine::SupportLevel::Supported;
                capabilities.supports_registry_push = susun_engine::SupportLevel::SupportedSubset;
                Ok(capabilities)
            }
        })
    }

    fn snapshot(&self, _project: &ProjectIdentity) -> BoxEngineFuture<'_, EngineSnapshot> {
        let fail = self.should_fail(EngineOperation::Snapshot);
        let snapshot = self
            .snapshot
            .clone()
            .unwrap_or_else(|| EngineSnapshot::empty(SystemTime::UNIX_EPOCH));
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::Snapshot))
            } else {
                Ok(snapshot)
            }
        })
    }

    fn container_inventory(&self) -> BoxEngineFuture<'_, EngineContainerInventory> {
        let fail = self.should_fail(EngineOperation::ContainerInventory);
        let inventory = self
            .container_inventory
            .clone()
            .unwrap_or(EngineContainerInventory {
                schema_version: EngineInventorySchemaVersion::CURRENT,
                observed_at_epoch_seconds: 0,
                containers: Vec::new(),
            });
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::ContainerInventory))
            } else {
                Ok(inventory)
            }
        })
    }

    fn container_details(&self, id: &ContainerId) -> BoxEngineFuture<'_, EngineContainerSummary> {
        let id = id.clone();
        let inventory = self.container_inventory();
        Box::pin(async move {
            inventory
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
        let fail = self.should_fail(EngineOperation::ImageInventory);
        let inventory = self
            .image_inventory
            .clone()
            .unwrap_or(EngineImageInventory {
                schema_version: EngineInventorySchemaVersion::CURRENT,
                observed_at_epoch_seconds: 0,
                images: Vec::new(),
            });
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::ImageInventory))
            } else {
                Ok(inventory)
            }
        })
    }

    fn image_details(&self, id: &susun_engine::ImageId) -> BoxEngineFuture<'_, EngineImageSummary> {
        let id = id.clone();
        let inventory = self.image_inventory();
        Box::pin(async move {
            inventory
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
        let fail = self.should_fail(EngineOperation::EngineInformation);
        let information = self
            .engine_information
            .clone()
            .unwrap_or(EngineInformation {
                schema_version: EngineInventorySchemaVersion::CURRENT,
                engine_version: None,
                operating_system: None,
                architecture: None,
                storage_driver: None,
                logical_cpus: None,
                memory_bytes: None,
                container_count: None,
                running_container_count: None,
                image_count: None,
            });
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::EngineInformation))
            } else {
                Ok(information)
            }
        })
    }

    fn pull_image(
        &self,
        request: PullImageRequest,
        _progress: ProgressSink,
    ) -> BoxEngineFuture<'_, EngineImageRef> {
        let fail = self.should_fail(EngineOperation::PullImage);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::PullImage))
            } else {
                Ok(EngineImageRef {
                    reference: request.image.as_str().to_owned(),
                })
            }
        })
    }

    fn remove_image(&self, request: ImageRemoveRequest) -> BoxEngineFuture<'_, ImageRemoveResult> {
        let fail = self.should_fail(EngineOperation::RemoveImage);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::RemoveImage))
            } else {
                Ok(ImageRemoveResult {
                    schema_version: ArtifactMutationSchemaVersion::CURRENT,
                    deleted: request
                        .image()
                        .as_str()
                        .starts_with("sha256:")
                        .then(|| susun_engine::ImageId::new(request.image().as_str().to_owned()))
                        .transpose()
                        .map_err(|error| EngineError::api(EngineOperation::RemoveImage, error))?
                        .into_iter()
                        .collect(),
                    untagged: (!request.image().as_str().starts_with("sha256:"))
                        .then(|| susun_model::ImageRef::new(request.image().as_str().to_owned()))
                        .into_iter()
                        .collect(),
                })
            }
        })
    }

    fn tag_image(&self, request: ImageTagRequest) -> BoxEngineFuture<'_, ImageTagResult> {
        let fail = self.should_fail(EngineOperation::TagImage);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::TagImage))
            } else {
                Ok(ImageTagResult {
                    schema_version: ArtifactMutationSchemaVersion::CURRENT,
                    source: request.source().clone(),
                    target: request.target().clone(),
                })
            }
        })
    }

    fn push_image(
        &self,
        request: ImagePushRequest,
        progress: ProgressSink,
    ) -> BoxEngineFuture<'_, ImagePushResult> {
        let fail = self.should_fail(EngineOperation::PushImage);
        Box::pin(async move {
            if fail {
                return Err(fake_error(EngineOperation::PushImage));
            }
            progress
                .emit(susun_engine::ActionProgress {
                    operation: EngineProgressOperation::PushImage,
                    stage: "push".to_owned(),
                    current: Some(1),
                    total: Some(1),
                    message: None,
                })
                .await;
            Ok(ImagePushResult {
                schema_version: ArtifactMutationSchemaVersion::CURRENT,
                image: request.image().clone(),
                digest: None,
            })
        })
    }

    fn create_network(&self, _request: CreateNetworkRequest) -> BoxEngineFuture<'_, NetworkRef> {
        let fail = self.should_fail(EngineOperation::CreateNetwork);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::CreateNetwork))
            } else {
                Ok(NetworkRef {
                    id: NetworkId::new("fake-network")
                        .map_err(|error| EngineError::api(EngineOperation::CreateNetwork, error))?,
                })
            }
        })
    }

    fn remove_network(&self, _id: NetworkRef) -> BoxEngineFuture<'_, ()> {
        unit(
            EngineOperation::RemoveNetwork,
            self.should_fail(EngineOperation::RemoveNetwork),
        )
    }

    fn create_volume(&self, _request: CreateVolumeRequest) -> BoxEngineFuture<'_, VolumeRef> {
        let fail = self.should_fail(EngineOperation::CreateVolume);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::CreateVolume))
            } else {
                Ok(VolumeRef {
                    id: VolumeId::new("fake-volume")
                        .map_err(|error| EngineError::api(EngineOperation::CreateVolume, error))?,
                })
            }
        })
    }

    fn remove_volume(&self, _id: VolumeRef) -> BoxEngineFuture<'_, ()> {
        unit(
            EngineOperation::RemoveVolume,
            self.should_fail(EngineOperation::RemoveVolume),
        )
    }

    fn create_container(
        &self,
        _request: CreateContainerRequest,
    ) -> BoxEngineFuture<'_, ContainerRef> {
        let fail = self.should_fail(EngineOperation::CreateContainer);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::CreateContainer))
            } else {
                Ok(ContainerRef {
                    id: ContainerId::new("fake-container").map_err(|error| {
                        EngineError::api(EngineOperation::CreateContainer, error)
                    })?,
                })
            }
        })
    }

    fn start_container(&self, _id: &ContainerRef) -> BoxEngineFuture<'_, ()> {
        unit(
            EngineOperation::StartContainer,
            self.should_fail(EngineOperation::StartContainer),
        )
    }

    fn stop_container(&self, _request: StopContainerRequest) -> BoxEngineFuture<'_, ()> {
        unit(
            EngineOperation::StopContainer,
            self.should_fail(EngineOperation::StopContainer),
        )
    }

    fn wait_container(
        &self,
        _request: WaitContainerRequest,
    ) -> BoxEngineFuture<'_, WaitContainerResult> {
        let fail = self.should_fail(EngineOperation::Wait);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::Wait))
            } else {
                Ok(WaitContainerResult { exit_code: 0 })
            }
        })
    }

    fn remove_container(
        &self,
        _id: &ContainerRef,
        _options: RemoveContainerOptions,
    ) -> BoxEngineFuture<'_, ()> {
        unit(
            EngineOperation::RemoveContainer,
            self.should_fail(EngineOperation::RemoveContainer),
        )
    }

    fn logs(&self, _request: LogsRequest) -> BoxEngineFuture<'_, BoxLogStream> {
        unsupported_or_fail(
            EngineOperation::Logs,
            self.should_fail(EngineOperation::Logs),
        )
    }

    fn events(&self, _request: EventsRequest) -> BoxEngineFuture<'_, BoxEventStream> {
        unsupported_or_fail(
            EngineOperation::Events,
            self.should_fail(EngineOperation::Events),
        )
    }

    fn exec(&self, _request: ExecRequest) -> BoxEngineFuture<'_, BoxExecStream> {
        unsupported_or_fail(
            EngineOperation::Exec,
            self.should_fail(EngineOperation::Exec),
        )
    }

    fn copy_from_container(
        &self,
        _request: CopyFromContainerRequest,
    ) -> BoxEngineFuture<'_, BoxByteStream> {
        unsupported_or_fail(
            EngineOperation::Copy,
            self.should_fail(EngineOperation::Copy),
        )
    }

    fn copy_to_container(&self, _request: CopyToContainerRequest) -> BoxEngineFuture<'_, ()> {
        unit(
            EngineOperation::Copy,
            self.should_fail(EngineOperation::Copy),
        )
    }

    fn port(&self, _request: PortRequest) -> BoxEngineFuture<'_, Vec<PublishedPortBinding>> {
        let fail = self.should_fail(EngineOperation::Port);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::Port))
            } else {
                Ok(Vec::new())
            }
        })
    }

    fn prune(&self, _request: PruneRequest) -> BoxEngineFuture<'_, PruneReport> {
        let fail = self.should_fail(EngineOperation::Prune);
        Box::pin(async move {
            if fail {
                Err(fake_error(EngineOperation::Prune))
            } else {
                Ok(PruneReport::default())
            }
        })
    }
}

fn unit(operation: EngineOperation, fail: bool) -> BoxEngineFuture<'static, ()> {
    Box::pin(async move {
        if fail {
            Err(fake_error(operation))
        } else {
            Ok(())
        }
    })
}

fn unsupported_or_fail<T>(operation: EngineOperation, fail: bool) -> BoxEngineFuture<'static, T> {
    Box::pin(async move {
        if fail {
            Err(fake_error(operation))
        } else {
            Err(EngineError::Unsupported {
                capability: operation_capability(operation),
            })
        }
    })
}

fn fake_error(operation: EngineOperation) -> EngineError {
    EngineError::api(operation, std::io::Error::other("fake engine failure"))
}

fn operation_capability(operation: EngineOperation) -> &'static str {
    match operation {
        EngineOperation::Capabilities => "capabilities",
        EngineOperation::Snapshot => "snapshot",
        EngineOperation::ContainerInventory => "container inventory",
        EngineOperation::ImageInventory => "image inventory",
        EngineOperation::EngineInformation => "engine information",
        EngineOperation::PullImage => "pull image",
        EngineOperation::RemoveImage => "remove image",
        EngineOperation::TagImage => "tag image",
        EngineOperation::PushImage => "push image",
        EngineOperation::CreateNetwork => "create network",
        EngineOperation::RemoveNetwork => "remove network",
        EngineOperation::CreateVolume => "create volume",
        EngineOperation::RemoveVolume => "remove volume",
        EngineOperation::CreateContainer => "create container",
        EngineOperation::StartContainer => "start container",
        EngineOperation::StopContainer => "stop container",
        EngineOperation::RemoveContainer => "remove container",
        EngineOperation::Logs => "logs",
        EngineOperation::Events => "events",
        EngineOperation::Exec => "exec",
        EngineOperation::Copy => "copy",
        EngineOperation::Port => "port",
        EngineOperation::Wait => "wait",
        EngineOperation::Prune => "prune",
    }
}
