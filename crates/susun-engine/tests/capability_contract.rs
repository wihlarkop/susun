#![allow(missing_docs)]

use susun_engine::{EngineCapabilities, SupportLevel};

#[test]
fn conservative_capabilities_do_not_advertise_uncallable_studio_operations() {
    let capabilities = EngineCapabilities::conservative();

    assert_eq!(
        capabilities.supports_container_inventory,
        SupportLevel::Unknown
    );
    assert_eq!(capabilities.supports_image_inventory, SupportLevel::Unknown);
    assert_eq!(
        capabilities.supports_engine_information,
        SupportLevel::Unknown
    );
    assert_eq!(
        capabilities.supports_image_management,
        SupportLevel::Unsupported
    );
    assert_eq!(capabilities.supports_registry_pull, SupportLevel::Unknown);
    assert_eq!(
        capabilities.supports_registry_push,
        SupportLevel::Unsupported
    );
    assert_eq!(capabilities.supports_build_cache, SupportLevel::Unsupported);
    assert_eq!(
        capabilities.supports_cleanup_preview,
        SupportLevel::Unsupported
    );
}

#[cfg(feature = "serde")]
#[test]
fn studio_capability_fields_have_stable_snake_case_json_names() -> Result<(), serde_json::Error> {
    let value = serde_json::to_value(EngineCapabilities::conservative())?;

    assert_eq!(value["supports_container_inventory"], "unknown");
    assert_eq!(value["supports_image_inventory"], "unknown");
    assert_eq!(value["supports_engine_information"], "unknown");
    assert_eq!(value["supports_image_management"], "unsupported");
    assert_eq!(value["supports_registry_pull"], "unknown");
    assert_eq!(value["supports_registry_push"], "unsupported");
    assert_eq!(value["supports_build_cache"], "unsupported");
    assert_eq!(value["supports_cleanup_preview"], "unsupported");
    Ok(())
}

#[cfg(feature = "serde")]
#[test]
fn older_capability_json_defaults_new_fields_to_unknown() -> Result<(), serde_json::Error> {
    let capabilities: EngineCapabilities = serde_json::from_value(serde_json::json!({
        "api_version": null,
        "supports_health": "supported",
        "supports_named_volumes": "supported",
        "supports_network_aliases": "supported_subset",
        "supports_mount_types": ["volume", "bind"],
        "supports_log_follow": "supported",
        "supports_build": "unsupported",
        "max_container_name_length": 255
    }))?;

    assert_eq!(
        capabilities.supports_container_inventory,
        SupportLevel::Unknown
    );
    assert_eq!(
        capabilities.supports_image_management,
        SupportLevel::Unknown
    );
    assert_eq!(capabilities.supports_cleanup_preview, SupportLevel::Unknown);
    Ok(())
}
