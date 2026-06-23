use susun_source::Spanned;

use super::{
    command::RawStringOrList, environment::RawMapping, port::RawPortEntry,
    volume::RawVolumeMount,
};

/// Raw parsed representation of a single service entry.
///
/// Fields absent in the YAML file use their [`Default`] value.
/// No parser-vendor types appear in this struct.
#[derive(Debug, Clone, Default)]
pub struct ParsedService {
    /// The `image:` field, if present.
    pub image: Option<Spanned<String>>,
    /// The `command:` field.
    pub command: RawStringOrList,
    /// The `entrypoint:` field.
    pub entrypoint: RawStringOrList,
    /// The `environment:` field.
    pub environment: RawMapping,
    /// The `labels:` field.
    pub labels: RawMapping,
    /// The `ports:` field.
    pub ports: Vec<RawPortEntry>,
    /// The `volumes:` field.
    pub volumes: Vec<RawVolumeMount>,
}
