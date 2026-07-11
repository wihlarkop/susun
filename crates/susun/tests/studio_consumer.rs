#![allow(missing_docs)]

use susun::{
    BuildImageIdentity, BuildResult, BuildResultSummary, ContainerEngine, EngineConnectionProfile,
    EngineConnectionProfileSet, ImagePushRequest, ProgressSink, PruneRequest, PruneScope,
    RegistryAuthMaterial, RegistryCredentialRef,
};
use susun_testkit::FakeContainerEngine;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn studio_runtime_artifact_registry_and_cleanup_flow_uses_only_the_facade() -> TestResult {
    let profiles = EngineConnectionProfileSet::new(vec![EngineConnectionProfile::local_default()])?;
    let selected = profiles
        .default_profile()
        .ok_or("missing selected runtime")?;
    assert_eq!(selected.id.as_str(), "local");

    let engine = FakeContainerEngine::new();
    let capabilities = engine.capabilities().await?;
    assert!(capabilities.supports_image_inventory.is_supported());
    assert!(capabilities.supports_registry_auth.is_supported());
    assert!(capabilities.supports_cleanup_preview.is_supported());

    let build = BuildResultSummary::from(&BuildResult {
        image: BuildImageIdentity {
            reference: "registry.example/studio/app:dev".to_owned(),
            digest: Some("sha256:build".to_owned()),
        },
    });
    assert_eq!(build.reference, "registry.example/studio/app:dev");

    let credential_ref = RegistryCredentialRef::new("studio:vault/registry")?;
    let pushed = engine
        .push_image_authenticated(
            ImagePushRequest::new(susun::ImageRef::new(&build.reference))
                .with_credential_ref(credential_ref.clone()),
            RegistryAuthMaterial::registry_token("ephemeral-secret"),
            ProgressSink::discard(),
        )
        .await?;
    assert_eq!(pushed.credential_ref, Some(credential_ref));

    let preview = engine
        .cleanup_preview(PruneRequest {
            scopes: vec![
                PruneScope::Containers,
                PruneScope::Images,
                PruneScope::BuildCache,
            ],
            all_images: false,
        })
        .await?;
    assert_eq!(preview.scopes.len(), 3);
    assert!(susun::render_cleanup_preview_json(&preview)?.contains("estimate_kind"));
    Ok(())
}
