//! Field-aware merge logic for individual service entries.

use indexmap::IndexMap;

use crate::input::{ParsedService, RawMapping, RawStringOrList};

use super::unique::{unique_ports, unique_volumes};

/// Merge two service entries, with `overlay` taking precedence over `base`.
///
/// Field semantics:
/// - `image`: overlay wins if present; otherwise base is kept.
/// - `command`, `entrypoint`: overlay wins unless it is `Null` (absent).
/// - `environment`, `labels`: key-level merge — overlay keys win.
/// - `ports`: concatenated, then deduplicated by canonical key (overlay wins).
/// - `volumes`: concatenated, then deduplicated by target path (overlay wins).
pub fn merge_services(base: ParsedService, overlay: ParsedService) -> ParsedService {
    ParsedService {
        image: overlay.image.or(base.image),
        build: overlay.build.or(base.build),
        command: merge_string_or_list(base.command, overlay.command),
        entrypoint: merge_string_or_list(base.entrypoint, overlay.entrypoint),
        environment: merge_mapping(base.environment, overlay.environment),
        labels: merge_mapping(base.labels, overlay.labels),
        ports: unique_ports(base.ports.into_iter().chain(overlay.ports).collect()),
        volumes: unique_volumes(base.volumes.into_iter().chain(overlay.volumes).collect()),
        depends_on: merge_depends_on(base.depends_on, overlay.depends_on),
        networks: merge_networks(base.networks, overlay.networks),
        configs: merge_resource_mounts(base.configs, overlay.configs),
        secrets: merge_resource_mounts(base.secrets, overlay.secrets),
        healthcheck: overlay.healthcheck.or(base.healthcheck),
        restart: overlay.restart.or(base.restart),
        profiles: if overlay.profiles.is_empty() {
            base.profiles
        } else {
            overlay.profiles
        },
    }
}

/// Overlay wins unless it is `Null` (which signals "absent").
fn merge_string_or_list(base: RawStringOrList, overlay: RawStringOrList) -> RawStringOrList {
    match overlay {
        RawStringOrList::Null => base,
        _ => overlay,
    }
}

/// Key-level merge: overlay keys win; base keys not in overlay are kept.
///
/// If both are map form after expansion, perform key-level merge.
/// If only overlay is a map form (or vice-versa), overlay wins entirely.
fn merge_mapping(base: RawMapping, overlay: RawMapping) -> RawMapping {
    match (base, overlay) {
        (RawMapping::Map(mut bmap), RawMapping::Map(omap)) => {
            for (k, v) in omap {
                bmap.insert(k, v);
            }
            RawMapping::Map(bmap)
        }
        (_, overlay) => overlay,
    }
}

fn merge_depends_on(
    mut base: crate::input::RawDependencies,
    overlay: crate::input::RawDependencies,
) -> crate::input::RawDependencies {
    for (k, v) in overlay {
        base.insert(k, v);
    }
    base
}

fn merge_networks(
    mut base: crate::input::RawServiceNetworks,
    overlay: crate::input::RawServiceNetworks,
) -> crate::input::RawServiceNetworks {
    for (k, v) in overlay {
        base.insert(k, v);
    }
    base
}

fn merge_resource_mounts(
    base: Vec<crate::input::RawResourceMount>,
    overlay: Vec<crate::input::RawResourceMount>,
) -> Vec<crate::input::RawResourceMount> {
    let mut merged: IndexMap<String, crate::input::RawResourceMount> = IndexMap::new();
    for mount in base.into_iter().chain(overlay) {
        let key = mount
            .target
            .as_ref()
            .map(|target| target.value.clone())
            .unwrap_or_else(|| mount.source.value.clone());
        merged.insert(key, mount);
    }
    merged.into_values().collect()
}
