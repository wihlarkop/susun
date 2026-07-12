#![allow(missing_docs)]
#![cfg(feature = "bollard")]

use susun::{ContainerEngine, DockerCompatibleEngine};

#[test]
fn docker_compatible_adapter_is_available_without_direct_adapter_imports() {
    fn assert_engine<T: ContainerEngine>() {}
    assert_engine::<DockerCompatibleEngine>();

    let constructor: fn() -> Result<DockerCompatibleEngine, susun::EngineConnectionError> =
        DockerCompatibleEngine::connect_local;
    let _ = constructor;
}
