//! Fake neutral container engine for runtime failure tests.

use std::time::SystemTime;

use susun_engine::{
    BoxByteStream, BoxEngineFuture, BoxEventStream, BoxExecStream, BoxLogStream, ContainerEngine,
    ContainerId, ContainerRef, CopyFromContainerRequest, CopyToContainerRequest,
    CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest, EngineCapabilities,
    EngineError, EngineImageRef, EngineOperation, EngineSnapshot, EventsRequest, ExecRequest,
    LogsRequest, NetworkId, NetworkRef, PortRequest, ProgressSink, ProjectIdentity, PruneReport,
    PruneRequest, PublishedPortBinding, PullImageRequest, RemoveContainerOptions,
    StopContainerRequest, VolumeId, VolumeRef, WaitContainerRequest, WaitContainerResult,
};

/// In-memory engine that can fail selected operations.
#[derive(Debug, Clone, Default)]
pub struct FakeContainerEngine {
    failures: Vec<EngineOperation>,
    snapshot: Option<EngineSnapshot>,
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
                Ok(EngineCapabilities::permissive_local())
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
        EngineOperation::PullImage => "pull image",
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
