//! Asynchronous neutral engine trait.

use std::{future::Future, pin::Pin};

use crate::{
    BoxByteStream, BoxEventStream, BoxExecStream, BoxLogStream, ContainerRef,
    CopyFromContainerRequest, CopyToContainerRequest, CreateContainerRequest, CreateNetworkRequest,
    CreateVolumeRequest, EngineCapabilities, EngineContainerInventory, EngineContainerSummary,
    EngineError, EngineImageInventory, EngineImageRef, EngineImageSummary, EngineInformation,
    EngineSnapshot, EventsRequest, ExecRequest, LogsRequest, NetworkRef, PortRequest, ProgressSink,
    ProjectIdentity, PruneReport, PruneRequest, PublishedPortBinding, PullImageRequest,
    RemoveContainerOptions, StopContainerRequest, VolumeRef, WaitContainerRequest,
    WaitContainerResult,
};

/// Boxed engine future.
pub type BoxEngineFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, EngineError>> + Send + 'a>>;

/// Runtime-executable neutral container engine.
pub trait ContainerEngine: Send + Sync {
    /// Returns engine capabilities.
    fn capabilities(&self) -> BoxEngineFuture<'_, EngineCapabilities>;

    /// Acquires a project-scoped engine snapshot.
    fn snapshot(&self, project: &ProjectIdentity) -> BoxEngineFuture<'_, EngineSnapshot>;

    /// Lists containers across the selected engine.
    fn container_inventory(&self) -> BoxEngineFuture<'_, EngineContainerInventory> {
        Box::pin(async {
            Err(EngineError::Unsupported {
                capability: "engine-wide container inventory",
            })
        })
    }

    /// Returns one container from the engine-wide inventory.
    fn container_details(
        &self,
        _id: &crate::ContainerId,
    ) -> BoxEngineFuture<'_, EngineContainerSummary> {
        Box::pin(async {
            Err(EngineError::Unsupported {
                capability: "engine-wide container details",
            })
        })
    }

    /// Lists images across the selected engine.
    fn image_inventory(&self) -> BoxEngineFuture<'_, EngineImageInventory> {
        Box::pin(async {
            Err(EngineError::Unsupported {
                capability: "engine-wide image inventory",
            })
        })
    }

    /// Returns one image from the engine-wide inventory.
    fn image_details(&self, _id: &crate::ImageId) -> BoxEngineFuture<'_, EngineImageSummary> {
        Box::pin(async {
            Err(EngineError::Unsupported {
                capability: "engine-wide image details",
            })
        })
    }

    /// Returns display-safe engine and host information.
    fn engine_information(&self) -> BoxEngineFuture<'_, EngineInformation> {
        Box::pin(async {
            Err(EngineError::Unsupported {
                capability: "engine information",
            })
        })
    }

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

    /// Waits for a container to exit.
    fn wait_container(
        &self,
        request: WaitContainerRequest,
    ) -> BoxEngineFuture<'_, WaitContainerResult>;

    /// Removes a container.
    fn remove_container(
        &self,
        id: &ContainerRef,
        options: RemoveContainerOptions,
    ) -> BoxEngineFuture<'_, ()>;

    /// Opens a neutral log stream.
    fn logs(&self, request: LogsRequest) -> BoxEngineFuture<'_, BoxLogStream>;

    /// Opens a project-scoped neutral engine event stream.
    fn events(&self, request: EventsRequest) -> BoxEngineFuture<'_, BoxEventStream>;

    /// Executes a command inside a running container and opens its output stream.
    fn exec(&self, request: ExecRequest) -> BoxEngineFuture<'_, BoxExecStream>;

    /// Copies a container path as a tar archive stream.
    fn copy_from_container(
        &self,
        request: CopyFromContainerRequest,
    ) -> BoxEngineFuture<'_, BoxByteStream>;

    /// Copies a tar archive into a container directory.
    fn copy_to_container(&self, request: CopyToContainerRequest) -> BoxEngineFuture<'_, ()>;

    /// Queries published host ports for a container.
    fn port(&self, request: PortRequest) -> BoxEngineFuture<'_, Vec<PublishedPortBinding>>;

    /// Runs a system-wide prune across the requested resource kinds. Unlike
    /// every other method on this trait, this is NOT scoped to a single
    /// project — it can remove containers, networks, volumes, and images
    /// belonging to ANY project or tool on the host engine.
    fn prune(&self, request: PruneRequest) -> BoxEngineFuture<'_, PruneReport>;
}
