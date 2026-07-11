#![allow(missing_docs)]

use susun::{
    ContainerEngine, ContainerId, ContainerState, EngineContainerInventory, EngineContainerSummary,
    EngineError, EngineImageInventory, EngineImageSummary, EngineInformation,
    EngineInventorySchemaVersion, HealthState, ImageId, ObservedImageRef, ResourceName,
    parse_engine_container_inventory_json, parse_engine_image_inventory_json,
    parse_engine_information_json, render_engine_container_inventory_json,
    render_engine_image_inventory_json, render_engine_information_json,
};
use susun_testkit::FakeContainerEngine;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn container(id: &str) -> TestResult<EngineContainerSummary> {
    Ok(EngineContainerSummary {
        id: ContainerId::new(id)?,
        name: ResourceName::new(format!("container-{id}"))?,
        state: ContainerState::Running,
        health: Some(HealthState::Healthy),
        image: ObservedImageRef::Unknown,
        label_keys: Vec::new(),
        project_identity: None,
        created_at_epoch_seconds: Some(1),
        writable_size_bytes: Some(2),
        root_filesystem_size_bytes: Some(3),
    })
}

fn image(id: &str) -> TestResult<EngineImageSummary> {
    Ok(EngineImageSummary {
        id: ImageId::new(id)?,
        references: vec![susun::ImageRef::new(format!("example/{id}:latest"))],
        digests: vec![format!("example/{id}@sha256:abc")],
        label_keys: Vec::new(),
        created_at_epoch_seconds: Some(1),
        size_bytes: Some(2),
        shared_size_bytes: None,
        container_count: Some(0),
    })
}

#[tokio::test]
async fn fake_engine_exposes_inventory_and_details() -> TestResult {
    let containers = EngineContainerInventory {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        observed_at_epoch_seconds: 10,
        containers: vec![container("container-a")?],
    };
    let images = EngineImageInventory {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        observed_at_epoch_seconds: 10,
        images: vec![image("image-a")?],
    };
    let information = EngineInformation {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        engine_version: None,
        operating_system: None,
        architecture: None,
        storage_driver: Some("overlay".to_owned()),
        logical_cpus: Some(4),
        memory_bytes: Some(1024),
        container_count: Some(1),
        running_container_count: Some(1),
        image_count: Some(1),
    };
    let engine = FakeContainerEngine::new()
        .with_container_inventory(containers.clone())
        .with_image_inventory(images.clone())
        .with_engine_information(information.clone());

    assert_eq!(engine.container_inventory().await?, containers);
    assert_eq!(
        engine
            .container_details(&ContainerId::new("container-a")?)
            .await?,
        containers.containers[0]
    );
    assert_eq!(engine.image_inventory().await?, images);
    assert_eq!(
        engine.image_details(&ImageId::new("image-a")?).await?,
        images.images[0]
    );
    assert_eq!(engine.engine_information().await?, information);
    Ok(())
}

#[tokio::test]
async fn fake_engine_inventory_failure_is_typed_and_redacted() -> TestResult {
    let engine = FakeContainerEngine::failing(susun::EngineOperation::ContainerInventory);
    let error = match engine.container_inventory().await {
        Ok(_) => {
            return Err(std::io::Error::other(
                "configured inventory failure unexpectedly succeeded",
            )
            .into());
        }
        Err(error) => error,
    };

    assert!(matches!(
        error,
        EngineError::Api {
            operation: susun::EngineOperation::ContainerInventory,
            ..
        }
    ));
    assert_eq!(
        error.redacted_message(),
        "engine container inventory failed"
    );
    Ok(())
}

#[test]
fn facade_inventory_json_helpers_roundtrip() -> TestResult {
    let containers = EngineContainerInventory {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        observed_at_epoch_seconds: 10,
        containers: vec![container("a")?, container("b")?],
    };
    let images = EngineImageInventory {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        observed_at_epoch_seconds: 10,
        images: vec![image("a")?, image("b")?],
    };
    let information = EngineInformation {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        engine_version: None,
        operating_system: None,
        architecture: None,
        storage_driver: Some("overlay".to_owned()),
        logical_cpus: Some(4),
        memory_bytes: Some(1024),
        container_count: Some(2),
        running_container_count: Some(2),
        image_count: Some(2),
    };

    assert_eq!(
        parse_engine_container_inventory_json(&render_engine_container_inventory_json(
            &containers,
        )?)?,
        containers
    );
    assert_eq!(
        parse_engine_image_inventory_json(&render_engine_image_inventory_json(&images)?)?,
        images
    );
    let json = render_engine_information_json(&information)?;
    assert!(!json.contains("docker_root"));
    assert!(!json.contains("registry_config"));
    assert_eq!(parse_engine_information_json(&json)?, information);
    Ok(())
}

#[test]
fn facade_inventory_json_rejects_schema_and_order_drift() -> TestResult {
    let inventory = EngineContainerInventory {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        observed_at_epoch_seconds: 10,
        containers: vec![container("b")?, container("a")?],
    };
    let json = render_engine_container_inventory_json(&inventory)?;
    assert!(parse_engine_container_inventory_json(&json).is_err());

    let mut value = serde_json::to_value(EngineImageInventory {
        schema_version: EngineInventorySchemaVersion::CURRENT,
        observed_at_epoch_seconds: 10,
        images: vec![image("a")?],
    })?;
    value["schema_version"]["major"] = serde_json::json!(2);
    assert!(parse_engine_image_inventory_json(&value.to_string()).is_err());
    Ok(())
}
