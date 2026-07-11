#![allow(missing_docs)]

use susun::{EngineCapabilities, SupportLevel};

#[test]
fn facade_exposes_neutral_studio_capability_contract() {
    let capabilities = EngineCapabilities::conservative();

    assert_eq!(
        capabilities.supports_container_inventory,
        SupportLevel::Unknown
    );
    assert_eq!(
        capabilities.supports_image_management,
        SupportLevel::Unsupported
    );
    assert_eq!(
        capabilities.supports_cleanup_preview,
        SupportLevel::Unsupported
    );
}
