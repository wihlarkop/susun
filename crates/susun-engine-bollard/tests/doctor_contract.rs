//! Runtime doctor contract tests that do not require a live Docker daemon.

use susun_engine::{
    EngineConnectionDisplayName, EngineConnectionProfile, EngineConnectionProfileId,
    EngineEndpoint, RuntimeDoctorStatus,
};

#[test]
fn profile_debug_output_redacts_endpoint() -> Result<(), Box<dyn std::error::Error>> {
    let profile = EngineConnectionProfile::new(
        EngineConnectionProfileId::new("local-dev")?,
        EngineConnectionDisplayName::new("Local development")?,
        EngineEndpoint::UnixSocket("/very/private/docker.sock".into()),
    );

    let debug = format!("{profile:?}");
    assert!(debug.contains("local-dev"));
    assert!(debug.contains("Local development"));
    assert!(debug.contains("unix://<local-socket>"));
    assert!(!debug.contains("very/private"));
    assert!(!debug.contains("docker.sock"));
    Ok(())
}

#[tokio::test]
async fn unavailable_profile_reports_redacted_unavailable() -> Result<(), Box<dyn std::error::Error>>
{
    let profile = EngineConnectionProfile::new(
        EngineConnectionProfileId::new("missing")?,
        EngineConnectionDisplayName::new("Missing runtime")?,
        EngineEndpoint::UnixSocket("/this/path/does/not/exist.sock".into()),
    );

    let report = susun_engine_bollard::BollardEngine::doctor_profile(&profile).await;

    assert_eq!(report.status, RuntimeDoctorStatus::Unavailable);
    assert_eq!(
        report.profile_id.as_ref().map(|id| id.as_str()),
        Some("missing")
    );
    assert_eq!(report.endpoint.to_string(), "unix://<local-socket>");
    assert!(report.probe.is_none());
    assert!(!report.message.contains("does/not/exist"));
    Ok(())
}
