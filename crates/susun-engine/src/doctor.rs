//! Neutral runtime readiness report types.

use crate::{
    EngineConnectionError, EngineConnectionProfileId, EngineEndpoint, EngineProbe, RedactedEndpoint,
};

/// High-level runtime readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RuntimeDoctorStatus {
    /// The endpoint connected and reported version information.
    Available,
    /// The endpoint could not be reached.
    Unavailable,
    /// The endpoint rejected authentication.
    AuthenticationFailed,
    /// The selected endpoint kind is unsupported on this platform.
    Unsupported,
    /// The endpoint/profile is malformed or incomplete.
    Misconfigured,
}

/// Redacted runtime readiness report suitable for logs, UI, and daemon APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RuntimeDoctorReport {
    /// Optional profile id when the check was profile-backed.
    pub profile_id: Option<EngineConnectionProfileId>,
    /// Readiness status.
    pub status: RuntimeDoctorStatus,
    /// Display-safe endpoint.
    pub endpoint: RedactedEndpoint,
    /// Engine probe details when available.
    pub probe: Option<EngineProbe>,
    /// Display-safe diagnostic message.
    pub message: String,
}

impl RuntimeDoctorReport {
    /// Builds an available report from a successful probe.
    pub fn available(
        profile_id: Option<EngineConnectionProfileId>,
        endpoint: &EngineEndpoint,
        probe: EngineProbe,
    ) -> Self {
        Self {
            profile_id,
            status: RuntimeDoctorStatus::Available,
            endpoint: RedactedEndpoint::new(endpoint),
            probe: Some(probe),
            message: "engine endpoint is available".to_owned(),
        }
    }

    /// Builds a redacted report from a connection/probe error.
    pub fn from_connection_error(
        profile_id: Option<EngineConnectionProfileId>,
        endpoint: &EngineEndpoint,
        error: &EngineConnectionError,
    ) -> Self {
        let status = match error {
            EngineConnectionError::InvalidEndpoint { .. }
            | EngineConnectionError::TlsConfiguration { .. } => RuntimeDoctorStatus::Misconfigured,
            EngineConnectionError::UnsupportedEndpoint { .. } => RuntimeDoctorStatus::Unsupported,
            EngineConnectionError::Authentication { .. } => {
                RuntimeDoctorStatus::AuthenticationFailed
            }
            EngineConnectionError::EndpointUnavailable { .. }
            | EngineConnectionError::ApiNegotiation { .. } => RuntimeDoctorStatus::Unavailable,
        };
        Self {
            profile_id,
            status,
            endpoint: RedactedEndpoint::new(endpoint),
            probe: None,
            message: error.to_string(),
        }
    }
}
