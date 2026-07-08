//! Endpoint construction and redaction contract tests — no Docker daemon
//! required for these; see the bottom of this file for the one live
//! connect_to+probe test, which skips gracefully with no daemon present.
//!
//! This workspace denies `clippy::unwrap_used`/`expect_used`/`panic`
//! project-wide, including in tests — so success-path tests return
//! `Result<(), _>` and use `?`, and error-path tests assert with
//! `matches!` rather than unwrapping.

mod support;

use susun_engine::{
    ClientIdentityFiles, EngineConnectionError, EngineEndpoint, EngineEndpointKind, EngineError,
    InvalidEngineEndpoint, TcpEndpoint, TlsConfiguration, TlsConfigurationError,
};

#[test]
fn local_endpoint_kind() {
    assert_eq!(EngineEndpoint::Local.kind(), EngineEndpointKind::Local);
}

#[test]
fn unix_socket_endpoint_kind_and_redaction() {
    let endpoint = EngineEndpoint::UnixSocket("/var/run/docker.sock".into());
    assert_eq!(endpoint.kind(), EngineEndpointKind::UnixSocket);
    assert_eq!(endpoint.redacted(), "unix://<local-socket>");
}

#[test]
fn windows_named_pipe_endpoint_kind_and_redaction() {
    let endpoint = EngineEndpoint::WindowsNamedPipe(r"\\.\pipe\docker_engine".into());
    assert_eq!(endpoint.kind(), EngineEndpointKind::WindowsNamedPipe);
    assert_eq!(endpoint.redacted(), "npipe://<local-pipe>");
}

#[test]
fn tcp_endpoint_host_and_port() -> Result<(), InvalidEngineEndpoint> {
    let tcp = TcpEndpoint::new("example.internal", 2375)?;
    assert_eq!(tcp.host(), "example.internal");
    assert_eq!(tcp.port(), 2375);
    assert!(tcp.tls().is_none());
    Ok(())
}

#[test]
fn tcp_endpoint_with_custom_ca() -> Result<(), InvalidEngineEndpoint> {
    let tls = TlsConfiguration::new().with_ca_certificate("C:/certs/ca.pem");
    let tcp = TcpEndpoint::new("example.internal", 2376)?.with_tls(tls);
    assert!(matches!(
        tcp.tls(),
        Some(tls) if tls.client_identity().is_none() && tls.ca_certificate().is_some()
    ));
    Ok(())
}

#[test]
fn tcp_endpoint_with_mutual_tls() -> Result<(), Box<dyn std::error::Error>> {
    let identity = ClientIdentityFiles::new("C:/certs/client.pem", "C:/certs/client-key.pem")?;
    let tls = TlsConfiguration::new()
        .with_ca_certificate("C:/certs/ca.pem")
        .with_client_identity(identity);
    let tcp = TcpEndpoint::new("example.internal", 2376)?.with_tls(tls);
    assert!(matches!(tcp.tls(), Some(tls) if tls.client_identity().is_some()));
    Ok(())
}

#[test]
fn tcp_endpoint_rejects_empty_host() {
    let result = TcpEndpoint::new("", 2375);
    assert!(matches!(result, Err(InvalidEngineEndpoint::EmptyHost)));
}

#[test]
fn tcp_endpoint_rejects_embedded_scheme() {
    let result = TcpEndpoint::new("http://example.internal", 2375);
    assert!(matches!(result, Err(InvalidEngineEndpoint::EmbeddedScheme)));
}

#[test]
fn tcp_endpoint_rejects_embedded_credentials() {
    let result = TcpEndpoint::new("user@example.internal", 2375);
    assert!(matches!(
        result,
        Err(InvalidEngineEndpoint::EmbeddedCredentials)
    ));
}

#[test]
fn tcp_endpoint_rejects_embedded_path() {
    let result = TcpEndpoint::new("example.internal/v2", 2375);
    assert!(matches!(
        result,
        Err(InvalidEngineEndpoint::EmbeddedPathOrQuery)
    ));
}

#[test]
fn tcp_endpoint_rejects_port_zero() {
    let result = TcpEndpoint::new("example.internal", 0);
    assert!(matches!(result, Err(InvalidEngineEndpoint::PortZero)));
}

#[test]
fn tcp_endpoint_accepts_bracketed_ipv6() -> Result<(), InvalidEngineEndpoint> {
    let tcp = TcpEndpoint::new("[::1]", 2375)?;
    assert_eq!(tcp.host(), "[::1]");
    Ok(())
}

#[test]
fn tcp_endpoint_rejects_malformed_bracketed_ipv6() {
    assert!(matches!(
        TcpEndpoint::new("[::1", 2375),
        Err(InvalidEngineEndpoint::MalformedIpv6)
    ));
    assert!(matches!(
        TcpEndpoint::new("[not-an-ipv6]", 2375),
        Err(InvalidEngineEndpoint::MalformedIpv6)
    ));
}

#[test]
fn tcp_endpoint_rejects_unbracketed_ipv6() {
    assert!(matches!(
        TcpEndpoint::new("::1", 2375),
        Err(InvalidEngineEndpoint::UnbracketedIpv6)
    ));
}

#[test]
fn client_identity_rejects_certificate_without_key() {
    let result = ClientIdentityFiles::new("C:/certs/client.pem", "");
    assert!(matches!(
        result,
        Err(TlsConfigurationError::IncompleteClientIdentity)
    ));
}

#[test]
fn client_identity_rejects_key_without_certificate() {
    let result = ClientIdentityFiles::new("", "C:/certs/client-key.pem");
    assert!(matches!(
        result,
        Err(TlsConfigurationError::IncompleteClientIdentity)
    ));
}

#[test]
fn endpoint_debug_output_is_redacted() -> Result<(), InvalidEngineEndpoint> {
    let endpoint = EngineEndpoint::UnixSocket("/var/run/docker.sock".into());
    let debug = format!("{endpoint:?}");
    assert!(!debug.contains("docker.sock"));

    let tcp_endpoint = EngineEndpoint::Tcp(TcpEndpoint::new("example.internal", 2375)?);
    let debug = format!("{tcp_endpoint:?}");
    assert!(!debug.contains("example.internal"));
    Ok(())
}

#[test]
fn tcp_endpoint_debug_output_is_redacted() -> Result<(), InvalidEngineEndpoint> {
    let tcp = TcpEndpoint::new("example.internal", 2375)?;
    let debug = format!("{tcp:?}");
    assert!(!debug.contains("example.internal"));
    assert!(debug.contains("2375"));
    Ok(())
}

#[test]
fn tls_configuration_debug_output_is_redacted() -> Result<(), Box<dyn std::error::Error>> {
    let identity = ClientIdentityFiles::new("C:/certs/client.pem", "C:/certs/client-key.pem")?;
    let tls = TlsConfiguration::new()
        .with_ca_certificate("C:/certs/ca.pem")
        .with_client_identity(identity);
    let debug = format!("{tls:?}");
    assert!(!debug.contains("client.pem"));
    assert!(!debug.contains("client-key.pem"));
    assert!(!debug.contains("ca.pem"));
    Ok(())
}

#[test]
fn bollard_adapter_rejects_tls_server_name_override() -> Result<(), Box<dyn std::error::Error>> {
    let identity = ClientIdentityFiles::new("C:/certs/client.pem", "C:/certs/client-key.pem")?;
    let tls = TlsConfiguration::new()
        .with_ca_certificate("C:/certs/ca.pem")
        .with_client_identity(identity)
        .with_server_name("docker.internal");
    let endpoint = EngineEndpoint::Tcp(TcpEndpoint::new("example.internal", 2376)?.with_tls(tls));

    let result = susun_engine_bollard::BollardEngine::connect_to(endpoint);

    assert!(matches!(
        result,
        Err(EngineConnectionError::TlsConfiguration { detail })
            if detail.contains("server-name override")
    ));
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn windows_named_pipe_is_unsupported_on_non_windows() {
    let endpoint = EngineEndpoint::WindowsNamedPipe(r"\\.\pipe\docker_engine".into());
    let result = susun_engine_bollard::BollardEngine::connect_to(endpoint);

    assert!(matches!(
        result,
        Err(EngineConnectionError::UnsupportedEndpoint {
            endpoint_kind: EngineEndpointKind::WindowsNamedPipe,
            ..
        })
    ));
}

#[cfg(windows)]
#[test]
fn windows_named_pipe_uses_named_pipe_endpoint_boundary() -> Result<(), EngineConnectionError> {
    let endpoint = EngineEndpoint::WindowsNamedPipe(r"\\.\pipe\missing_susun_test_pipe".into());
    let engine = susun_engine_bollard::BollardEngine::connect_to(endpoint)?;

    assert_eq!(
        engine.endpoint().kind(),
        EngineEndpointKind::WindowsNamedPipe
    );
    Ok(())
}

#[tokio::test]
async fn connect_to_local_and_probe() -> Result<(), EngineError> {
    let Some(_engine) = support::docker_engine().await? else {
        return Ok(());
    };
    // support::docker_engine() already exercises BollardEngine::connect_local
    // internally; this test exercises connect_to(EngineEndpoint::Local)
    // explicitly, plus probe(), against the same real local daemon.
    let engine = susun_engine_bollard::BollardEngine::connect_to(EngineEndpoint::Local)
        .map_err(EngineError::Connection)?;
    let probe = engine.probe().await.map_err(EngineError::Connection)?;
    assert!(probe.api_version.is_some() || probe.engine_version.is_some());
    Ok(())
}

#[test]
fn connect_to_unavailable_endpoint_maps_to_endpoint_unavailable() {
    let endpoint = EngineEndpoint::UnixSocket("/this/path/does/not/exist.sock".into());
    let result = susun_engine_bollard::BollardEngine::connect_to(endpoint);
    assert!(matches!(
        result,
        Err(EngineConnectionError::EndpointUnavailable { .. })
    ));
}
