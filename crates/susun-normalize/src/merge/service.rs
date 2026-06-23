//! Field-aware merge logic for individual service entries.

use indexmap::IndexMap;
use susun_source::Spanned;

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
        command: merge_string_or_list(base.command, overlay.command),
        entrypoint: merge_string_or_list(base.entrypoint, overlay.entrypoint),
        environment: merge_mapping(base.environment, overlay.environment),
        labels: merge_mapping(base.labels, overlay.labels),
        ports: unique_ports(base.ports.into_iter().chain(overlay.ports).collect()),
        volumes: unique_volumes(base.volumes.into_iter().chain(overlay.volumes).collect()),
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

/// Merge a `Spanned<String>` scalar: overlay wins if present.
pub(super) fn merge_scalar(
    base: Option<Spanned<String>>,
    overlay: Option<Spanned<String>>,
) -> Option<Spanned<String>> {
    overlay.or(base)
}

/// Merge an `IndexMap`: overlay entries win per key; base keys not in overlay survive.
pub(super) fn merge_map(
    mut base: IndexMap<String, Option<Spanned<String>>>,
    overlay: IndexMap<String, Option<Spanned<String>>>,
) -> IndexMap<String, Option<Spanned<String>>> {
    for (k, v) in overlay {
        base.insert(k, v);
    }
    base
}
