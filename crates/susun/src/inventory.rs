//! Engine-wide inventory JSON contracts for SDK consumers.

use serde::de::Error as _;

use crate::{
    EngineContainerInventory, EngineImageInventory, EngineInformation, EngineInventorySchemaVersion,
};

/// Renders container inventory as pretty JSON.
pub fn render_engine_container_inventory_json(
    inventory: &EngineContainerInventory,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(inventory)
}

/// Parses and validates container inventory JSON.
pub fn parse_engine_container_inventory_json(
    input: &str,
) -> Result<EngineContainerInventory, serde_json::Error> {
    let inventory: EngineContainerInventory = serde_json::from_str(input)?;
    validate_schema(inventory.schema_version)?;
    if inventory
        .containers
        .windows(2)
        .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
    {
        return Err(serde_json::Error::custom(
            "container inventory must use unique ascending engine IDs",
        ));
    }
    Ok(inventory)
}

/// Renders image inventory as pretty JSON.
pub fn render_engine_image_inventory_json(
    inventory: &EngineImageInventory,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(inventory)
}

/// Parses and validates image inventory JSON.
pub fn parse_engine_image_inventory_json(
    input: &str,
) -> Result<EngineImageInventory, serde_json::Error> {
    let inventory: EngineImageInventory = serde_json::from_str(input)?;
    validate_schema(inventory.schema_version)?;
    if inventory
        .images
        .windows(2)
        .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
    {
        return Err(serde_json::Error::custom(
            "image inventory must use unique ascending engine IDs",
        ));
    }
    Ok(inventory)
}

/// Renders display-safe engine information as pretty JSON.
pub fn render_engine_information_json(
    information: &EngineInformation,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(information)
}

/// Parses and validates display-safe engine information JSON.
pub fn parse_engine_information_json(input: &str) -> Result<EngineInformation, serde_json::Error> {
    let information: EngineInformation = serde_json::from_str(input)?;
    validate_schema(information.schema_version)?;
    Ok(information)
}

fn validate_schema(version: EngineInventorySchemaVersion) -> Result<(), serde_json::Error> {
    if version != EngineInventorySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported engine inventory schema version {}.{}",
            version.major, version.minor
        )));
    }
    Ok(())
}
