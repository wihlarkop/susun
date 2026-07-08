//! Runtime profile JSON helpers for SDK consumers.

use serde::{Deserialize, Serialize, de::Error as _};
use susun_engine::{EngineConnectionProfileSet, EngineEndpointKind, RedactedEndpoint};

/// Serializable, redacted runtime profile set summary for UI/API consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineConnectionProfileSetSummary {
    /// Serialized profile summary schema version.
    pub schema_version: EngineConnectionProfileSetSummarySchemaVersion,
    /// Profile summaries in configured order.
    pub profiles: Vec<EngineConnectionProfileSummary>,
    /// Selected default profile id, if the set has any profiles.
    pub default_profile_id: Option<String>,
}

/// Serialized profile summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineConnectionProfileSetSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl EngineConnectionProfileSetSummarySchemaVersion {
    /// Current profile summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Serializable, redacted runtime profile summary for UI/API consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineConnectionProfileSummary {
    /// Stable profile id.
    pub id: String,
    /// User-visible display name.
    pub display_name: String,
    /// Endpoint kind without sensitive endpoint contents.
    pub endpoint_kind: EngineEndpointKind,
    /// Display-safe endpoint token.
    pub redacted_endpoint: RedactedEndpoint,
    /// Whether this profile was explicitly marked default.
    pub default: bool,
}

impl From<&EngineConnectionProfileSet> for EngineConnectionProfileSetSummary {
    fn from(profiles: &EngineConnectionProfileSet) -> Self {
        Self {
            schema_version: EngineConnectionProfileSetSummarySchemaVersion::CURRENT,
            profiles: profiles
                .profiles()
                .iter()
                .map(EngineConnectionProfileSummary::from)
                .collect(),
            default_profile_id: profiles
                .default_profile()
                .map(|profile| profile.id.as_str().to_owned()),
        }
    }
}

impl From<&susun_engine::EngineConnectionProfile> for EngineConnectionProfileSummary {
    fn from(profile: &susun_engine::EngineConnectionProfile) -> Self {
        Self {
            id: profile.id.as_str().to_owned(),
            display_name: profile.display_name.as_str().to_owned(),
            endpoint_kind: profile.endpoint().kind(),
            redacted_endpoint: RedactedEndpoint::new(profile.endpoint()),
            default: profile.is_default(),
        }
    }
}

/// Renders an engine connection profile set as pretty JSON.
///
/// This is configuration JSON, not a redacted UI summary. Endpoint fields can
/// include local socket paths, named pipes, remote hosts, and TLS file paths.
/// Store it only in protected application storage.
pub fn render_engine_connection_profile_set_json(
    profiles: &EngineConnectionProfileSet,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(profiles)
}

/// Parses an engine connection profile set from JSON.
///
/// Deserialization uses the validated Susun profile types, so invalid profile
/// ids, empty display names, duplicate ids, and multiple defaults are rejected.
/// This is configuration JSON, not a redacted UI summary. Endpoint fields can
/// include local socket paths, named pipes, remote hosts, and TLS file paths.
/// Read it only from protected application storage.
pub fn parse_engine_connection_profile_set_json(
    input: &str,
) -> Result<EngineConnectionProfileSet, serde_json::Error> {
    serde_json::from_str(input)
}

/// Renders a redacted engine connection profile summary as pretty JSON.
pub fn render_engine_connection_profile_set_summary_json(
    summary: &EngineConnectionProfileSetSummary,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a redacted engine connection profile summary from JSON.
pub fn parse_engine_connection_profile_set_summary_json(
    input: &str,
) -> Result<EngineConnectionProfileSetSummary, serde_json::Error> {
    let summary: EngineConnectionProfileSetSummary = serde_json::from_str(input)?;
    validate_engine_connection_profile_set_summary(&summary)?;
    Ok(summary)
}

fn validate_engine_connection_profile_set_summary(
    summary: &EngineConnectionProfileSetSummary,
) -> Result<(), serde_json::Error> {
    if summary.schema_version != EngineConnectionProfileSetSummarySchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported engine connection profile set summary schema version {}.{}",
            summary.schema_version.major, summary.schema_version.minor
        )));
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut explicit_defaults = 0usize;
    for profile in &summary.profiles {
        if profile.id.is_empty() {
            return Err(serde_json::Error::custom(
                "engine connection profile summary id must not be empty",
            ));
        }
        if !ids.insert(profile.id.as_str()) {
            return Err(serde_json::Error::custom(
                "engine connection profile summary contains duplicate ids",
            ));
        }
        explicit_defaults += usize::from(profile.default);
    }
    if explicit_defaults > 1 {
        return Err(serde_json::Error::custom(
            "engine connection profile summary contains multiple defaults",
        ));
    }
    match &summary.default_profile_id {
        Some(default) if !ids.contains(default.as_str()) => {
            return Err(serde_json::Error::custom(
                "engine connection profile summary default id is not present",
            ));
        }
        Some(default) => {
            let explicit_default = summary
                .profiles
                .iter()
                .find(|profile| profile.default)
                .map(|profile| profile.id.as_str());
            if explicit_default.is_some_and(|id| id != default) {
                return Err(serde_json::Error::custom(
                    "engine connection profile summary default flags do not match default id",
                ));
            }
        }
        None if !summary.profiles.is_empty() => {
            return Err(serde_json::Error::custom(
                "engine connection profile summary default id is required when profiles exist",
            ));
        }
        None => {}
    }
    Ok(())
}
