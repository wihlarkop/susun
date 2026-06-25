//! Deduplication helpers for ports and volumes.
//!
//! Port uniqueness key: (host_ip, target, published, protocol).
//! Volume uniqueness key: target path.
//! In both cases the last entry with a given key wins.

use indexmap::IndexMap;

use susun_model::{CanonicalPort, PublishedPort};

use crate::{
    input::{port::RawPortEntry, volume::RawVolumeMount},
    port::parse_port_entry,
    volume::parse_volume_entry,
};

/// Deduplicate a list of raw port entries by canonical key.
///
/// Key: `(host_ip, target, published, protocol)`. When two entries share a
/// key, the later one wins. Entries that fail to parse are kept as-is under a
/// unique synthetic key so they survive for the validation pass.
pub fn unique_ports(ports: Vec<RawPortEntry>) -> Vec<RawPortEntry> {
    // We use an IndexMap so the insertion order of the LAST winning entry is
    // preserved relative to other keys. Keys that appear only once keep their
    // position; keys that appear multiple times keep the position of the last.
    //
    // Strategy: accumulate into a map keyed by port key string, then collect
    // in insertion order.
    let mut map: IndexMap<PortKey, RawPortEntry> = IndexMap::new();
    let mut fallback_idx: u64 = 0;

    for entry in ports {
        let key = match parse_port_entry(&entry) {
            Ok(canonical) => PortKey::Canonical(canonical_port_key(&canonical)),
            Err(_) => {
                // Unparseable entry — keep under a unique key.
                fallback_idx += 1;
                PortKey::Fallback(fallback_idx)
            }
        };
        map.insert(key, entry);
    }

    map.into_values().collect()
}

/// Deduplicate a list of raw volume entries by target path.
///
/// When two entries share the same target, the later one wins.
/// Entries that fail to parse are kept as-is under a unique synthetic key.
pub fn unique_volumes(volumes: Vec<RawVolumeMount>) -> Vec<RawVolumeMount> {
    let mut map: IndexMap<VolumeKey, RawVolumeMount> = IndexMap::new();
    let mut fallback_idx: u64 = 0;

    for entry in volumes {
        let key = match parse_volume_entry(&entry) {
            Ok(canonical) => VolumeKey::Target(canonical.target),
            Err(_) => {
                fallback_idx += 1;
                VolumeKey::Fallback(fallback_idx)
            }
        };
        map.insert(key, entry);
    }

    map.into_values().collect()
}

// ── Keys ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PortKey {
    Canonical((Option<String>, u16, Option<PortRangeKey>, u8)),
    Fallback(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PortRangeKey {
    Single(u16),
    Range(u16, u16),
}

fn canonical_port_key(p: &CanonicalPort) -> (Option<String>, u16, Option<PortRangeKey>, u8) {
    let published_key = p.published.map(|pub_| match pub_ {
        PublishedPort::Single(n) => PortRangeKey::Single(n),
        PublishedPort::Range { start, end } => PortRangeKey::Range(start, end),
    });
    let protocol_byte = p.protocol as u8;
    (p.host_ip.clone(), p.target, published_key, protocol_byte)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum VolumeKey {
    Target(String),
    Fallback(u64),
}
