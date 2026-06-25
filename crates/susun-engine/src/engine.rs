//! Asynchronous neutral engine trait.

use std::{future::Future, pin::Pin};

use crate::{
    BoxLogStream, ContainerRef, CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest,
    EngineCapabilities, EngineError, EngineImageRef, EngineSnapshot, LogsRequest, NetworkRef,
    ProgressSink, ProjectIdentity, PullImageRequest, RemoveContainerOptions, StopContainerRequest,
    VolumeRef,
};

/// Boxed engine future.
pub type BoxEngineFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, EngineError>> + Send + 'a>>;

/// Runtime-executable neutral container engine.
pub trait ContainerEngine: Send + Sync {
    /// Returns engine capabilities.
    fn capabilities(&self) -> BoxEngineFuture<'_, EngineCapabilities>;

    /// Acquires a project-scoped engine snapshot.
    fn snapshot(&self, project: &ProjectIdentity) -> BoxEngineFuture<'_, EngineSnapshot>;

    /// Pulls an image.
    fn pull_image(
        &self,
        request: PullImageRequest,
        progress: ProgressSink,
    ) -> BoxEngineFuture<'_, EngineImageRef>;

    /// Creates a network.
    fn create_network(&self, request: CreateNetworkRequest) -> BoxEngineFuture<'_, NetworkRef>;

    /// Removes a network.
    fn remove_network(&self, id: NetworkRef) -> BoxEngineFuture<'_, ()>;

    /// Creates a volume.
    fn create_volume(&self, request: CreateVolumeRequest) -> BoxEngineFuture<'_, VolumeRef>;

    /// Removes a volume.
    fn remove_volume(&self, id: VolumeRef) -> BoxEngineFuture<'_, ()>;

    /// Creates a container.
    fn create_container(
        &self,
        request: CreateContainerRequest,
    ) -> BoxEngineFuture<'_, ContainerRef>;

    /// Starts a container.
    fn start_container(&self, id: &ContainerRef) -> BoxEngineFuture<'_, ()>;

    /// Stops a container.
    fn stop_container(&self, request: StopContainerRequest) -> BoxEngineFuture<'_, ()>;

    /// Removes a container.
    fn remove_container(
        &self,
        id: &ContainerRef,
        options: RemoveContainerOptions,
    ) -> BoxEngineFuture<'_, ()>;

    /// Opens a neutral log stream.
    fn logs(&self, request: LogsRequest) -> BoxEngineFuture<'_, BoxLogStream>;
}
