//! Runtime profile JSON helpers for SDK consumers.

use susun_engine::EngineConnectionProfileSet;

/// Renders an engine connection profile set as pretty JSON.
pub fn render_engine_connection_profile_set_json(
    profiles: &EngineConnectionProfileSet,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(profiles)
}

/// Parses an engine connection profile set from JSON.
///
/// Deserialization uses the validated Susun profile types, so invalid profile
/// ids, empty display names, duplicate ids, and multiple defaults are rejected.
pub fn parse_engine_connection_profile_set_json(
    input: &str,
) -> Result<EngineConnectionProfileSet, serde_json::Error> {
    serde_json::from_str(input)
}
