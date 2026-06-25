//! Volume mount parser: converts raw volume entries to canonical model types.
//!
//! Handles named volumes `"mydata:/data"`, bind mounts `"/host:/container:ro"`,
//! anonymous mounts `"/data"`, and the long-form YAML mapping.

use thiserror::Error;

use susun_model::{CanonicalVolume, VolumeKind};

use crate::input::volume::{RawVolumeLong, RawVolumeMount, RawVolumeShort};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors produced when parsing a volume mount string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VolumeParseError {
    /// The string does not match any recognised volume syntax.
    #[error("invalid volume format: {0:?}")]
    InvalidFormat(String),
    /// The target path is empty or otherwise invalid.
    #[error("volume target path must be absolute or non-empty: {0:?}")]
    InvalidTarget(String),
    /// The volume type is not recognised.
    #[error("unknown volume type {0:?} (expected bind, volume, or tmpfs)")]
    UnknownType(String),
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Parse a [`RawVolumeMount`] into a [`CanonicalVolume`].
pub fn parse_volume_entry(entry: &RawVolumeMount) -> Result<CanonicalVolume, VolumeParseError> {
    match entry {
        RawVolumeMount::Short(short) => parse_short(short),
        RawVolumeMount::Long(long) => parse_long(long),
    }
}

// ── Short-form parser ─────────────────────────────────────────────────────────

/// Parse a short-form volume string such as `"/host:/container:ro"`.
pub fn parse_short(short: &RawVolumeShort) -> Result<CanonicalVolume, VolumeParseError> {
    parse_short_str(short.0.value.as_str())
}

fn parse_short_str(s: &str) -> Result<CanonicalVolume, VolumeParseError> {
    let parts: Vec<&str> = s.splitn(3, ':').collect();

    match parts.len() {
        1 => {
            let target = validate_target(parts[0])?;
            Ok(CanonicalVolume {
                kind: VolumeKind::Anonymous,
                source: None,
                target,
                read_only: false,
            })
        }
        2 => {
            let source = parts[0].to_owned();
            let target = validate_target(parts[1])?;
            let kind = infer_kind(&source);
            Ok(CanonicalVolume {
                kind,
                source: Some(source),
                target,
                read_only: false,
            })
        }
        3 => {
            let source = parts[0].to_owned();
            let target = validate_target(parts[1])?;
            let read_only = parse_options(parts[2], s)?;
            let kind = infer_kind(&source);
            Ok(CanonicalVolume {
                kind,
                source: Some(source),
                target,
                read_only,
            })
        }
        _ => Err(VolumeParseError::InvalidFormat(s.to_owned())),
    }
}

// ── Long-form parser ──────────────────────────────────────────────────────────

/// Parse a long-form volume mount from explicit YAML fields.
pub fn parse_long(long: &RawVolumeLong) -> Result<CanonicalVolume, VolumeParseError> {
    let target = match &long.target {
        Some(t) => validate_target(t.value.as_str())?,
        None => {
            return Err(VolumeParseError::InvalidTarget(
                "long-form volume missing `target`".to_owned(),
            ));
        }
    };

    let kind = match &long.volume_type {
        Some(t) => parse_volume_type(t.value.as_str())?,
        None => VolumeKind::Volume,
    };

    let source = long.source.as_ref().map(|s| s.value.clone());
    let read_only = match &long.read_only {
        Some(r) => parse_bool_str(r.value.as_str()),
        None => false,
    };

    Ok(CanonicalVolume {
        kind,
        source,
        target,
        read_only,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn validate_target(s: &str) -> Result<String, VolumeParseError> {
    if s.is_empty() {
        return Err(VolumeParseError::InvalidTarget(s.to_owned()));
    }
    Ok(s.to_owned())
}

/// Infer whether a source string is a host path (starts with `/`, `./`, or `../`)
/// or a named volume name.
pub fn infer_kind(source: &str) -> VolumeKind {
    if source.starts_with('/') || source.starts_with("./") || source.starts_with("../") {
        VolumeKind::Bind
    } else {
        VolumeKind::Volume
    }
}

fn parse_options(options: &str, full: &str) -> Result<bool, VolumeParseError> {
    let mut read_only = false;
    for opt in options.split(',') {
        match opt.trim() {
            "ro" | "readonly" => read_only = true,
            "rw" => read_only = false,
            "" => {}
            _ => return Err(VolumeParseError::InvalidFormat(full.to_owned())),
        }
    }
    Ok(read_only)
}

fn parse_volume_type(s: &str) -> Result<VolumeKind, VolumeParseError> {
    match s {
        "volume" => Ok(VolumeKind::Volume),
        "bind" => Ok(VolumeKind::Bind),
        "tmpfs" => Ok(VolumeKind::Anonymous),
        other => Err(VolumeParseError::UnknownType(other.to_owned())),
    }
}

fn parse_bool_str(s: &str) -> bool {
    matches!(s, "true" | "1" | "yes")
}
