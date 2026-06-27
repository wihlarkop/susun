use susun_source::Spanned;

use super::{
    build::RawBuildDefinition,
    command::RawStringOrList,
    dependency::RawDependencies,
    environment::RawMapping,
    health::RawHealthcheck,
    port::RawPortEntry,
    resource::{RawResourceMount, RawServiceNetworks},
    volume::RawVolumeMount,
};

/// Raw parsed representation of a single service entry.
///
/// Fields absent in the YAML file use their [`Default`] value.
/// No parser-vendor types appear in this struct.
#[derive(Debug, Clone, Default)]
pub struct ParsedService {
    /// Service merge behavior marker.
    pub merge_tag: ServiceMergeTag,
    /// The `image:` field, if present.
    pub image: Option<Spanned<String>>,
    /// The `build:` field, if present.
    pub build: Option<RawBuildDefinition>,
    /// The `extends:` field, if present.
    pub extends: Option<RawExtends>,
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
    /// The `depends_on:` field.
    pub depends_on: RawDependencies,
    /// The `networks:` field.
    pub networks: RawServiceNetworks,
    /// The `configs:` field.
    pub configs: Vec<RawResourceMount>,
    /// The `secrets:` field.
    pub secrets: Vec<RawResourceMount>,
    /// The `healthcheck:` field.
    pub healthcheck: Option<RawHealthcheck>,
    /// The `restart:` field.
    pub restart: Option<Spanned<String>>,
    /// The `profiles:` field.
    pub profiles: Vec<Spanned<String>>,
}

/// Raw service `extends` declaration.
#[derive(Debug, Clone)]
pub struct RawExtends {
    /// Source service name.
    pub service: Spanned<String>,
    /// Optional source file path.
    pub file: Option<Spanned<String>>,
}

/// Advanced Compose merge tag applied to a service value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServiceMergeTag {
    /// Normal field-aware merge.
    #[default]
    Merge,
    /// Reset inherited/base service before applying this service.
    Reset,
    /// Override inherited/base service completely.
    Override,
}
