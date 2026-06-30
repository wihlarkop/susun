//! Minimal read-only `ContainerEngine` adapter skeleton.

use std::{process::ExitCode, time::SystemTime};

use susun_engine::{
    BoxByteStream, BoxEngineFuture, BoxEventStream, BoxExecStream, BoxLogStream, ContainerEngine,
    ContainerRef, CopyFromContainerRequest, CopyToContainerRequest, CreateContainerRequest,
    CreateNetworkRequest, CreateVolumeRequest, EngineCapabilities, EngineError, EngineImageRef,
    EngineSnapshot, EventsRequest, ExecRequest, LogsRequest, NetworkRef, PortRequest, ProgressSink,
    ProjectIdentity, PublishedPortBinding, PullImageRequest, RemoveContainerOptions,
    StopContainerRequest, VolumeRef, WaitContainerRequest, WaitContainerResult,
};
use susun_model::ProjectName;

#[derive(Debug, Default)]
struct ReadOnlyEngine;

impl ContainerEngine for ReadOnlyEngine {
    fn capabilities(&self) -> BoxEngineFuture<'_, EngineCapabilities> {
        Box::pin(async { Ok(EngineCapabilities::conservative()) })
    }

    fn snapshot(&self, _project: &ProjectIdentity) -> BoxEngineFuture<'_, EngineSnapshot> {
        Box::pin(async { Ok(EngineSnapshot::empty(SystemTime::UNIX_EPOCH)) })
    }

    fn pull_image(
        &self,
        _request: PullImageRequest,
        _progress: ProgressSink,
    ) -> BoxEngineFuture<'_, EngineImageRef> {
        unsupported("pull image")
    }

    fn create_network(&self, _request: CreateNetworkRequest) -> BoxEngineFuture<'_, NetworkRef> {
        unsupported("create network")
    }

    fn remove_network(&self, _id: NetworkRef) -> BoxEngineFuture<'_, ()> {
        unsupported("remove network")
    }

    fn create_volume(&self, _request: CreateVolumeRequest) -> BoxEngineFuture<'_, VolumeRef> {
        unsupported("create volume")
    }

    fn remove_volume(&self, _id: VolumeRef) -> BoxEngineFuture<'_, ()> {
        unsupported("remove volume")
    }

    fn create_container(
        &self,
        _request: CreateContainerRequest,
    ) -> BoxEngineFuture<'_, ContainerRef> {
        unsupported("create container")
    }

    fn start_container(&self, _id: &ContainerRef) -> BoxEngineFuture<'_, ()> {
        unsupported("start container")
    }

    fn stop_container(&self, _request: StopContainerRequest) -> BoxEngineFuture<'_, ()> {
        unsupported("stop container")
    }

    fn wait_container(
        &self,
        _request: WaitContainerRequest,
    ) -> BoxEngineFuture<'_, WaitContainerResult> {
        unsupported("wait container")
    }

    fn remove_container(
        &self,
        _id: &ContainerRef,
        _options: RemoveContainerOptions,
    ) -> BoxEngineFuture<'_, ()> {
        unsupported("remove container")
    }

    fn logs(&self, _request: LogsRequest) -> BoxEngineFuture<'_, BoxLogStream> {
        unsupported("logs")
    }

    fn events(&self, _request: EventsRequest) -> BoxEngineFuture<'_, BoxEventStream> {
        unsupported("events")
    }

    fn exec(&self, _request: ExecRequest) -> BoxEngineFuture<'_, BoxExecStream> {
        unsupported("exec")
    }

    fn copy_from_container(
        &self,
        _request: CopyFromContainerRequest,
    ) -> BoxEngineFuture<'_, BoxByteStream> {
        unsupported("copy from container")
    }

    fn copy_to_container(&self, _request: CopyToContainerRequest) -> BoxEngineFuture<'_, ()> {
        unsupported("copy to container")
    }

    fn port(&self, _request: PortRequest) -> BoxEngineFuture<'_, Vec<PublishedPortBinding>> {
        unsupported("port")
    }
}

fn unsupported<T>(capability: &'static str) -> BoxEngineFuture<'static, T> {
    Box::pin(async move { Err(EngineError::Unsupported { capability }) })
}

#[tokio::main]
async fn main() -> ExitCode {
    let engine = ReadOnlyEngine;
    let project = ProjectName::new("custom-engine-example");
    let identity = ProjectIdentity::new(
        project.clone(),
        susun_engine::ProjectInstanceId::derive(&project, "."),
    );

    match engine.snapshot(&identity).await {
        Ok(snapshot) => {
            println!("observed containers {}", snapshot.containers.len());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
