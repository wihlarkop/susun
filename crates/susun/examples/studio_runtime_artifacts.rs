//! Studio-style artifact calls through the public facade and testkit.

use susun::{
    ContainerEngine, ImagePushRequest, ProgressSink, PruneRequest, PruneScope,
    RegistryAuthMaterial, RegistryCredentialRef,
};
use susun_testkit::FakeContainerEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = FakeContainerEngine::new();
    let credential_ref = RegistryCredentialRef::new("app-vault:registry/example")?;
    let push = ImagePushRequest::new(susun::ImageRef::new("registry.example/team/app:dev"))
        .with_credential_ref(credential_ref);

    // Resolve the reference in the embedding application's credential vault.
    // Auth material is ephemeral and cannot be serialized by Susun.
    engine
        .push_image_authenticated(
            push,
            RegistryAuthMaterial::registry_token("resolved-at-call-time"),
            ProgressSink::discard(),
        )
        .await?;

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
    println!("{}", susun::render_cleanup_preview_json(&preview)?);
    Ok(())
}
