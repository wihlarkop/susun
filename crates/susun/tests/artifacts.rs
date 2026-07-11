#![allow(missing_docs)]

use std::sync::{Arc, Mutex};

use susun::{
    ArtifactMutationSchemaVersion, BuildImageIdentity, BuildResult, BuildResultSummary,
    ContainerEngine, EngineOperation, EngineProgressOperation, ImagePushRequest,
    ImageRemoveRequest, ImageSelector, ImageTagRequest, ProgressSink,
    parse_build_result_summary_json, parse_image_push_result_json, parse_image_remove_result_json,
    parse_image_tag_result_json, render_build_result_summary_json, render_image_push_result_json,
    render_image_remove_result_json, render_image_tag_result_json,
};
use susun_testkit::FakeContainerEngine;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn fake_engine_runs_image_mutations_and_emits_typed_push_progress() -> TestResult {
    let engine = FakeContainerEngine::new();
    let removed = engine
        .remove_image(ImageRemoveRequest::new(ImageSelector::new(
            "example/app:old",
        )?))
        .await?;
    assert_eq!(removed.untagged[0].as_str(), "example/app:old");

    let tagged = engine
        .tag_image(ImageTagRequest::new(
            ImageSelector::new("sha256:abc")?,
            susun::ImageRef::new("example/app:new"),
        ))
        .await?;
    assert_eq!(tagged.target.as_str(), "example/app:new");

    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let progress = ProgressSink::new(move |event| {
        let captured = Arc::clone(&captured);
        Box::pin(async move {
            if let Ok(mut events) = captured.lock() {
                events.push(event);
            }
        })
    });
    let pushed = engine
        .push_image(
            ImagePushRequest::new(susun::ImageRef::new("example/app:new")),
            progress,
        )
        .await?;
    assert_eq!(pushed.image.as_str(), "example/app:new");
    let events = events
        .lock()
        .map_err(|_| std::io::Error::other("progress lock was poisoned"))?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].operation, EngineProgressOperation::PushImage);
    Ok(())
}

#[tokio::test]
async fn fake_engine_mutation_failures_are_typed_and_display_safe() -> TestResult {
    let engine = FakeContainerEngine::failing(EngineOperation::TagImage);
    let result = engine
        .tag_image(ImageTagRequest::new(
            ImageSelector::new("sha256:abc")?,
            susun::ImageRef::new("example/app:new"),
        ))
        .await;
    let error = match result {
        Ok(_) => return Err(std::io::Error::other("tag unexpectedly succeeded").into()),
        Err(error) => error,
    };
    assert_eq!(error.redacted_message(), "engine tag image failed");
    Ok(())
}

#[test]
fn facade_artifact_results_roundtrip_and_reject_future_schema() -> TestResult {
    let removed = susun::ImageRemoveResult {
        schema_version: ArtifactMutationSchemaVersion::CURRENT,
        deleted: vec![susun::ImageId::new("sha256:abc")?],
        untagged: vec![susun::ImageRef::new("example/app:old")],
    };
    let tagged = susun::ImageTagResult {
        schema_version: ArtifactMutationSchemaVersion::CURRENT,
        source: ImageSelector::new("sha256:abc")?,
        target: susun::ImageRef::new("example/app:new"),
    };
    let pushed = susun::ImagePushResult {
        schema_version: ArtifactMutationSchemaVersion::CURRENT,
        image: susun::ImageRef::new("example/app:new"),
        digest: Some("sha256:def".to_owned()),
        credential_ref: None,
    };
    let build = BuildResultSummary::from(&BuildResult {
        image: BuildImageIdentity {
            reference: "example/app:new".to_owned(),
            digest: Some("sha256:def".to_owned()),
        },
    });

    assert_eq!(
        parse_image_remove_result_json(&render_image_remove_result_json(&removed)?)?,
        removed
    );
    assert_eq!(
        parse_image_tag_result_json(&render_image_tag_result_json(&tagged)?)?,
        tagged
    );
    assert_eq!(
        parse_image_push_result_json(&render_image_push_result_json(&pushed)?)?,
        pushed
    );
    assert_eq!(
        parse_build_result_summary_json(&render_build_result_summary_json(&build)?)?,
        build
    );

    let mut value = serde_json::to_value(&pushed)?;
    value["schema_version"]["major"] = serde_json::json!(2);
    assert!(parse_image_push_result_json(&value.to_string()).is_err());
    assert!(serde_json::from_str::<ImageSelector>(r#"""#).is_err());
    Ok(())
}

#[tokio::test]
async fn authenticated_push_exposes_only_the_credential_reference() -> TestResult {
    let secret = "studio-secret-token";
    let credential_ref = susun::RegistryCredentialRef::new("studio:vault/registry-1")?;
    let request = ImagePushRequest::new(susun::ImageRef::new("registry.example/app:new"))
        .with_credential_ref(credential_ref.clone());
    let request_json = serde_json::to_string(&request)?;
    assert!(request_json.contains(credential_ref.as_str()));
    assert!(!request_json.contains(secret));

    let auth =
        susun::RegistryAuthMaterial::registry_token(secret).with_server_address("registry.example");
    let debug = format!("{auth:?}");
    assert!(!debug.contains(secret));
    assert!(!debug.contains("registry.example"));
    assert!(debug.contains("[redacted]"));

    let auth_error = susun::EngineError::Authentication {
        registry: "<registry>".to_owned(),
    };
    assert_eq!(
        auth_error.to_string(),
        "engine authentication failed for <registry>"
    );
    assert!(!format!("{auth_error:?}").contains(secret));

    let result = FakeContainerEngine::new()
        .push_image_authenticated(request, auth, ProgressSink::discard())
        .await?;
    assert_eq!(result.credential_ref, Some(credential_ref));
    let result_json = render_image_push_result_json(&result)?;
    assert!(!result_json.contains(secret));
    Ok(())
}

#[tokio::test]
async fn cleanup_preview_is_separate_from_prune_and_roundtrips() -> TestResult {
    let request = susun::PruneRequest {
        scopes: vec![
            susun::PruneScope::Containers,
            susun::PruneScope::Images,
            susun::PruneScope::BuildCache,
        ],
        all_images: false,
    };
    let preview = FakeContainerEngine::new()
        .cleanup_preview(request.clone())
        .await?;
    assert_eq!(preview.request, request);
    assert_eq!(preview.scopes.len(), 3);
    let json = susun::render_cleanup_preview_json(&preview)?;
    assert_eq!(susun::parse_cleanup_preview_json(&json)?, preview);
    let mut future = serde_json::to_value(&preview)?;
    future["schema_version"]["major"] = serde_json::json!(2);
    assert!(susun::parse_cleanup_preview_json(&future.to_string()).is_err());
    Ok(())
}
