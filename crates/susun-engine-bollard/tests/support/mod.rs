//! Docker integration test support.

use std::time::Duration;
use susun_engine::{
    ContainerEngine, EngineError, ProjectIdentity, RemoveContainerOptions, StopContainerRequest,
};
use susun_engine_bollard::BollardEngine;

/// Returns true when Docker integration is mandatory for this test process.
pub fn docker_required() -> bool {
    std::env::var_os("SUSUN_DOCKER_REQUIRED").is_some()
}

/// Returns a local Docker engine when available, otherwise skips the caller.
pub async fn docker_engine() -> Result<Option<BollardEngine>, EngineError> {
    let required = docker_required();
    let engine = match BollardEngine::connect_local() {
        Ok(engine) => engine,
        Err(error) => {
            if required {
                return Err(error);
            }
            eprintln!("skipping Docker integration test: {error}");
            return Ok(None);
        }
    };
    if let Err(error) = engine.capabilities().await {
        if required {
            return Err(error);
        }
        eprintln!("skipping Docker integration test: {error}");
        return Ok(None);
    }
    Ok(Some(engine))
}

/// Best-effort cleanup for all resources owned by one project.
pub async fn cleanup_project(
    engine: &BollardEngine,
    project: &ProjectIdentity,
) -> Result<(), EngineError> {
    let snapshot = engine.snapshot(project).await?;

    for container in snapshot.containers.values() {
        let container_ref = susun_engine::ContainerRef {
            id: container.id.clone(),
        };
        let _ = engine
            .stop_container(StopContainerRequest {
                container: container_ref.clone(),
                timeout: Duration::from_secs(1),
            })
            .await;
        let _ = engine
            .remove_container(
                &container_ref,
                RemoveContainerOptions {
                    remove_anonymous_volumes: true,
                    force: true,
                },
            )
            .await;
    }

    for network in snapshot.networks.values() {
        let _ = engine
            .remove_network(susun_engine::NetworkRef {
                id: network.id.clone(),
            })
            .await;
    }

    for volume in snapshot.volumes.values() {
        let _ = engine
            .remove_volume(susun_engine::VolumeRef {
                id: volume.id.clone(),
            })
            .await;
    }

    Ok(())
}
