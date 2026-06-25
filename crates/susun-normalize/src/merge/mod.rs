//! Field-aware merge of Compose projects.
//!
//! `merge_projects(base, overlay)` applies overlay on top of base using
//! per-field strategies rather than a generic deep merge:
//!
//! | Field | Strategy |
//! |---|---|
//! | `name` | overlay wins if present |
//! | `image` | overlay wins if present |
//! | `command`, `entrypoint` | overlay wins unless absent (`Null`) |
//! | `environment`, `labels` | key-level merge, overlay per key wins |
//! | `ports` | concat then dedup by canonical key |
//! | `volumes` | concat then dedup by target path |

pub mod service;
pub mod unique;

pub use service::merge_services;

use susun_source::Spanned;

use crate::input::{MergeProject, ParsedService};

/// Merge `overlay` on top of `base`.
///
/// Services present in both are merged field-by-field. Services only in
/// `overlay` are added. Services only in `base` are kept unchanged.
pub fn merge_projects(base: MergeProject, overlay: MergeProject) -> MergeProject {
    let mut services = base.services;

    for (name, overlay_svc) in overlay.services {
        let span = overlay_svc.span;
        let merged_svc: ParsedService = if let Some(base_svc) = services.swap_remove(&name) {
            merge_services(base_svc.value, overlay_svc.value)
        } else {
            overlay_svc.value
        };
        services.insert(name, Spanned::new(merged_svc, span));
    }

    MergeProject {
        name: overlay.name.or(base.name),
        services,
        networks: merge_resource_map(base.networks, overlay.networks),
        volumes: merge_resource_map(base.volumes, overlay.volumes),
        configs: merge_resource_map(base.configs, overlay.configs),
        secrets: merge_resource_map(base.secrets, overlay.secrets),
    }
}

fn merge_resource_map(
    mut base: crate::input::RawResources,
    overlay: crate::input::RawResources,
) -> crate::input::RawResources {
    for (name, definition) in overlay {
        base.insert(name, definition);
    }
    base
}
