//! Bollard adapter contract tests.

mod support;

use futures_util::StreamExt;
use indexmap::IndexMap;
use std::time::Duration;
use susun_engine::{
    ContainerEngine, CreateContainerRequest, EngineError, LabelKey, LabelValue, LogsRequest,
    PullImageRequest, PullPolicy, RemoveContainerOptions, ReplicaIndex, ResourceName,
    ServiceInstanceId, StopContainerRequest,
};
use susun_model::{
    Command, ImageRef, NetworkAttachment, ServiceName, port::CanonicalPort, volume::CanonicalVolume,
};
use susun_testkit::{
    ContractProject, assert_basic_engine_contract, assert_resource_lifecycle_contract,
    engine_contract::{ownership_labels, unique_suffix},
};

#[tokio::test]
async fn bollard_satisfies_basic_engine_contract() -> Result<(), EngineError> {
    let Some(engine) = support::docker_engine().await? else {
        return Ok(());
    };
    assert_basic_engine_contract(&engine).await
}

#[tokio::test]
async fn bollard_creates_snapshots_and_removes_project_resources() -> Result<(), EngineError> {
    let Some(engine) = support::docker_engine().await? else {
        return Ok(());
    };
    let project = ContractProject::new(&unique_suffix("resources"))?;
    let result = assert_resource_lifecycle_contract(&engine, &project).await;
    let cleanup = support::cleanup_project(&engine, &project.identity).await;
    result?;
    cleanup
}

#[tokio::test]
async fn bollard_runs_prebuilt_container_and_streams_logs() -> Result<(), EngineError> {
    if cfg!(windows) && !support::docker_required() {
        eprintln!("skipping Linux image lifecycle contract on optional Windows platform job");
        return Ok(());
    }
    let Some(engine) = support::docker_engine().await? else {
        return Ok(());
    };
    let project = ContractProject::new(&unique_suffix("container"))?;
    let result = run_container_contract(&engine, &project).await;
    let cleanup = support::cleanup_project(&engine, &project.identity).await;
    result?;
    cleanup
}

async fn run_container_contract(
    engine: &susun_engine_bollard::BollardEngine,
    project: &ContractProject,
) -> Result<(), EngineError> {
    let image = ImageRef::new("busybox:1.36.1");
    engine
        .pull_image(
            PullImageRequest {
                image: image.clone(),
                policy: PullPolicy::Missing,
            },
            susun_engine::ProgressSink::discard(),
        )
        .await?;

    let resources = assert_resource_lifecycle_contract(engine, project).await?;
    let mut labels = ownership_labels(project)?;
    labels.insert(label_key("io.susun.service")?, label_value("web")?);
    labels.insert(label_key("io.susun.replica")?, label_value("1")?);

    let service = ServiceInstanceId::new(
        project.identity.working_set.clone(),
        ServiceName::new("web"),
        ReplicaIndex::FIRST,
    );
    let container = engine
        .create_container(CreateContainerRequest {
            project: project.identity.clone(),
            service,
            name: ResourceName::new(format!("{}-container", project.network_name.as_str()))
                .map_err(|error| {
                    EngineError::api(susun_engine::EngineOperation::CreateContainer, error)
                })?,
            image: Some(image),
            command: Some(Command::Exec(vec![
                "sh".to_owned(),
                "-c".to_owned(),
                "echo susun-contract-log; sleep 20".to_owned(),
            ])),
            entrypoint: None,
            environment: IndexMap::from([("SUSUN_CONTRACT".to_owned(), Some("true".to_owned()))]),
            container_labels: IndexMap::from([(
                "io.susun.contract-kind".to_owned(),
                "container".to_owned(),
            )]),
            ports: Vec::<CanonicalPort>::new(),
            volumes: Vec::<CanonicalVolume>::new(),
            networks: IndexMap::from([(
                project.network_name.clone(),
                NetworkAttachment {
                    aliases: vec!["web".to_owned()],
                },
            )]),
            healthcheck: None,
            restart: Some("no".to_owned()),
            labels,
        })
        .await?;

    engine.start_container(&container).await?;

    let snapshot = engine.snapshot(&project.identity).await?;
    if !snapshot
        .containers
        .values()
        .any(|observed| observed.id == container.id)
    {
        return Err(EngineError::Unsupported {
            capability: "created container was not visible in snapshot",
        });
    }

    let mut logs = engine
        .logs(LogsRequest {
            container: container.clone(),
            follow: false,
            timestamps: false,
            tail: Some(10),
        })
        .await?;
    let mut saw_log_line = false;
    while let Some(event) = logs.next().await {
        if event?.line.contains("susun-contract-log") {
            saw_log_line = true;
            break;
        }
    }
    if !saw_log_line {
        return Err(EngineError::Unsupported {
            capability: "container log line was not streamed",
        });
    }

    engine
        .stop_container(StopContainerRequest {
            container: container.clone(),
            timeout: Duration::from_secs(1),
        })
        .await?;
    engine
        .remove_container(
            &container,
            RemoveContainerOptions {
                remove_anonymous_volumes: true,
                force: true,
            },
        )
        .await?;
    engine.remove_network(resources.network).await?;
    engine.remove_volume(resources.volume).await
}

fn label_key(value: &str) -> Result<LabelKey, EngineError> {
    LabelKey::new(value)
        .map_err(|error| EngineError::api(susun_engine::EngineOperation::CreateContainer, error))
}

fn label_value(value: &str) -> Result<LabelValue, EngineError> {
    LabelValue::new(value)
        .map_err(|error| EngineError::api(susun_engine::EngineOperation::CreateContainer, error))
}
