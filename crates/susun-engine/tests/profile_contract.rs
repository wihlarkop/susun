#![allow(missing_docs)]

use susun_engine::{
    EngineConnectionDisplayName, EngineConnectionProfile, EngineConnectionProfileError,
    EngineConnectionProfileId, EngineConnectionProfileSet, EngineEndpoint, EngineError,
    EngineOperation, RuntimeDoctorReport, RuntimeDoctorStatus,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn profile(
    id: &str,
    display_name: &str,
    endpoint: EngineEndpoint,
) -> Result<EngineConnectionProfile, EngineConnectionProfileError> {
    Ok(EngineConnectionProfile::new(
        EngineConnectionProfileId::new(id)?,
        EngineConnectionDisplayName::new(display_name)?,
        endpoint,
    ))
}

#[test]
fn local_default_profile_is_selected_and_redacted() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![EngineConnectionProfile::local_default()])?;
    let default = set.default_profile().ok_or("expected default profile")?;

    assert_eq!(default.id.as_str(), "local");
    assert!(default.is_default());
    assert_eq!(default.redacted_endpoint(), "local");
    Ok(())
}

#[test]
fn duplicate_profile_ids_are_rejected() -> TestResult {
    let result = EngineConnectionProfileSet::new(vec![
        profile("local", "Local A", EngineEndpoint::Local)?,
        profile("local", "Local B", EngineEndpoint::Local)?,
    ]);

    assert!(matches!(
        result,
        Err(EngineConnectionProfileError::DuplicateId(id)) if id.as_str() == "local"
    ));
    Ok(())
}

#[test]
fn multiple_defaults_are_rejected() -> TestResult {
    let result = EngineConnectionProfileSet::new(vec![
        profile("one", "One", EngineEndpoint::Local)?.with_default(true),
        profile("two", "Two", EngineEndpoint::Local)?.with_default(true),
    ]);

    assert!(matches!(
        result,
        Err(EngineConnectionProfileError::MultipleDefaults)
    ));
    Ok(())
}

#[test]
fn first_profile_is_default_when_none_marked() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![
        profile("one", "One", EngineEndpoint::Local)?,
        profile("two", "Two", EngineEndpoint::Local)?,
    ])?;

    let default = set.default_profile().ok_or("expected default profile")?;
    assert_eq!(default.id.as_str(), "one");
    Ok(())
}

#[test]
fn profile_set_lookup_uses_profile_id() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![
        profile("one", "One", EngineEndpoint::Local)?,
        profile("two", "Two", EngineEndpoint::Local)?.with_default(true),
    ])?;
    let id = EngineConnectionProfileId::new("two")?;

    let selected = set.get(&id).ok_or("expected profile")?;
    assert_eq!(selected.display_name.as_str(), "Two");
    assert_eq!(
        set.default_profile().ok_or("expected default")?.id.as_str(),
        "two"
    );
    Ok(())
}

#[test]
fn profile_debug_redacts_endpoint() -> TestResult {
    let profile = profile(
        "private",
        "Private socket",
        EngineEndpoint::UnixSocket("/very/private/docker.sock".into()),
    )?;
    let debug = format!("{profile:?}");

    assert!(debug.contains("unix://<local-socket>"));
    assert!(!debug.contains("very/private"));
    assert!(!debug.contains("docker.sock"));
    Ok(())
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_invalid_profile_id() {
    let json = r#"{
        "profiles": [
            {
                "id": "bad id",
                "display_name": "Bad",
                "endpoint": "Local",
                "default": false
            }
        ]
    }"#;

    let result = serde_json::from_str::<EngineConnectionProfileSet>(json);
    assert!(result.is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_duplicate_profile_ids() {
    let json = r#"{
        "profiles": [
            {
                "id": "local",
                "display_name": "Local A",
                "endpoint": "Local",
                "default": false
            },
            {
                "id": "local",
                "display_name": "Local B",
                "endpoint": "Local",
                "default": false
            }
        ]
    }"#;

    let result = serde_json::from_str::<EngineConnectionProfileSet>(json);
    assert!(result.is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_multiple_default_profiles() {
    let json = r#"{
        "profiles": [
            {
                "id": "one",
                "display_name": "One",
                "endpoint": "Local",
                "default": true
            },
            {
                "id": "two",
                "display_name": "Two",
                "endpoint": "Local",
                "default": true
            }
        ]
    }"#;

    let result = serde_json::from_str::<EngineConnectionProfileSet>(json);
    assert!(result.is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serde_roundtrips_valid_profile_set() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![EngineConnectionProfile::local_default()])?;

    let json = serde_json::to_string(&set)?;
    let parsed: EngineConnectionProfileSet = serde_json::from_str(&json)?;

    assert_eq!(
        parsed
            .default_profile()
            .ok_or("expected default profile")?
            .id
            .as_str(),
        "local"
    );
    Ok(())
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_arbitrary_redacted_endpoint_text() -> TestResult {
    let result = serde_json::from_str::<susun_engine::RedactedEndpoint>(
        r#""this-is-not-actually-redacted""#,
    );

    assert!(result.is_err());

    let parsed =
        serde_json::from_str::<susun_engine::RedactedEndpoint>(r#""unix://<local-socket>""#)?;
    assert_eq!(parsed.to_string(), "unix://<local-socket>");
    Ok(())
}

#[cfg(feature = "serde")]
#[test]
fn serde_reuses_tcp_endpoint_validation() {
    let result = serde_json::from_value::<susun_engine::TcpEndpoint>(serde_json::json!({
        "host": "::1",
        "port": 2375
    }));

    assert!(result.is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serde_reuses_client_identity_validation() {
    let result = serde_json::from_value::<susun_engine::ClientIdentityFiles>(serde_json::json!({
        "certificate": "C:/certs/client.pem",
        "private_key": ""
    }));

    assert!(result.is_err());
}

#[test]
fn doctor_report_does_not_surface_raw_api_negotiation_source() {
    #[derive(Debug)]
    struct RawSource;

    impl std::fmt::Display for RawSource {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("raw /very/private/docker.sock source")
        }
    }

    impl std::error::Error for RawSource {}

    let endpoint = EngineEndpoint::UnixSocket("/very/private/docker.sock".into());
    let error = susun_engine::EngineConnectionError::ApiNegotiation {
        source: Box::new(RawSource),
    };

    let report = RuntimeDoctorReport::from_connection_error(None, &endpoint, &error);

    assert_eq!(report.status, RuntimeDoctorStatus::Unavailable);
    assert_eq!(report.message, "failed to probe engine API version");
    assert!(!report.message.contains("very/private"));
    assert!(!report.message.contains("docker.sock"));
}

#[test]
fn engine_error_redacted_message_does_not_surface_raw_api_source() {
    #[derive(Debug)]
    struct RawSource;

    impl std::fmt::Display for RawSource {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("raw /very/private/docker.sock source")
        }
    }

    impl std::error::Error for RawSource {}

    let error = EngineError::api(EngineOperation::PullImage, RawSource);
    let message = error.redacted_message();

    assert_eq!(message, "engine pull image failed");
    assert!(!message.contains("very/private"));
    assert!(!message.contains("docker.sock"));
}
