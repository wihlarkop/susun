//! Expands raw parsed forms into merge-ready representations.
//!
//! The main task is normalising both YAML forms of `environment` and `labels`
//! (mapping and sequence) into the canonical map form so that [`merge`][crate::merge]
//! can operate on a single consistent representation.

use indexmap::IndexMap;
use susun_source::Spanned;

use crate::input::{
    MergeProject, ParsedProject, ParsedService, RawMapping, RawStringOrList, port::RawPortEntry,
    volume::RawVolumeMount,
};

/// Expand a raw parsed project into a merge-ready project.
///
/// Each service's `environment` and `labels` list forms are converted to map
/// form. All other fields pass through unchanged.
pub fn expand_project(parsed: ParsedProject) -> MergeProject {
    MergeProject {
        name: parsed.name,
        services: parsed
            .services
            .into_iter()
            .map(|(name, spanned)| {
                let expanded = expand_service(spanned.value);
                (name, Spanned::new(expanded, spanned.span))
            })
            .collect(),
        networks: parsed.networks,
        volumes: parsed.volumes,
        configs: parsed.configs,
        secrets: parsed.secrets,
    }
}

fn expand_service(svc: ParsedService) -> ParsedService {
    ParsedService {
        image: svc.image,
        command: expand_command(svc.command),
        entrypoint: expand_command(svc.entrypoint),
        environment: expand_mapping(svc.environment),
        labels: expand_mapping(svc.labels),
        ports: expand_ports(svc.ports),
        volumes: expand_volumes(svc.volumes),
        depends_on: svc.depends_on,
        networks: svc.networks,
        configs: svc.configs,
        secrets: svc.secrets,
        healthcheck: svc.healthcheck,
        restart: svc.restart,
        profiles: svc.profiles,
    }
}

/// Command and entrypoint pass through unchanged; expansion is a no-op.
fn expand_command(cmd: RawStringOrList) -> RawStringOrList {
    cmd
}

/// Normalise a [`RawMapping`] to map form.
///
/// The sequence form `- KEY=value` / `- KEY` is parsed into `(key, Option<value>)`
/// pairs. Duplicate keys retain the last entry; merge-time conflict resolution
/// happens in the merge step (Task 22).
fn expand_mapping(mapping: RawMapping) -> RawMapping {
    match mapping {
        RawMapping::Map(_) => mapping,
        RawMapping::List(entries) => {
            let mut map: IndexMap<String, Option<Spanned<String>>> = IndexMap::new();
            for entry in entries {
                let (key, value) = split_env_entry(entry);
                map.insert(key, value);
            }
            RawMapping::Map(map)
        }
    }
}

/// Ports pass through unchanged; canonical parsing happens in Task 20.
fn expand_ports(ports: Vec<RawPortEntry>) -> Vec<RawPortEntry> {
    ports
}

/// Volumes pass through unchanged; canonical parsing happens in Task 21.
fn expand_volumes(volumes: Vec<RawVolumeMount>) -> Vec<RawVolumeMount> {
    volumes
}

/// Split a `"KEY=value"` or `"KEY"` entry into `(key, Option<Spanned<value>>)`.
///
/// The span of the value is set to the span of the full entry because the
/// adapter does not track sub-string positions.
fn split_env_entry(entry: Spanned<String>) -> (String, Option<Spanned<String>>) {
    match entry.value.find('=') {
        Some(eq) => {
            let key = entry.value[..eq].to_owned();
            let val = Spanned::new(entry.value[eq + 1..].to_owned(), entry.span);
            (key, Some(val))
        }
        None => (entry.value, None),
    }
}
