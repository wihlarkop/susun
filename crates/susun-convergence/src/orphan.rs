//! Conservative orphan classification.

use susun_engine::{
    NetworkIdentity, ObservedContainer, ObservedNetwork, ObservedVolume, VolumeIdentity,
};

use crate::OrphanPolicy;

/// Classified orphan resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrphanResource {
    /// Susun-owned container not present in desired state.
    Container(ObservedContainer),
    /// Susun-owned network not present in desired state.
    Network(ObservedNetwork),
    /// Susun-owned volume not present in desired state.
    Volume(ObservedVolume),
}

/// Decision for an orphan resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanDisposition {
    /// Report the orphan without removing it.
    Report,
    /// Remove the orphan.
    Remove,
    /// Ignore the orphan.
    Ignore,
}

/// Classifies container orphans from ownership indexing.
pub fn classify_orphan_containers(
    containers: &[ObservedContainer],
    policy: &OrphanPolicy,
) -> Vec<(OrphanResource, OrphanDisposition)> {
    containers
        .iter()
        .cloned()
        .map(|container| {
            let disposition = if policy.remove_containers {
                OrphanDisposition::Remove
            } else if policy.report {
                OrphanDisposition::Report
            } else {
                OrphanDisposition::Ignore
            };
            (OrphanResource::Container(container), disposition)
        })
        .collect()
}

/// Classifies network orphans by excluding desired identities.
pub fn classify_orphan_networks(
    observed: impl Iterator<Item = ObservedNetwork>,
    desired: &indexmap::IndexSet<NetworkIdentity>,
    policy: &OrphanPolicy,
) -> Vec<(OrphanResource, OrphanDisposition)> {
    observed
        .filter(|network| {
            network
                .network_identity
                .as_ref()
                .is_some_and(|id| !desired.contains(id))
        })
        .map(|network| {
            let disposition = if policy.remove_networks {
                OrphanDisposition::Remove
            } else if policy.report {
                OrphanDisposition::Report
            } else {
                OrphanDisposition::Ignore
            };
            (OrphanResource::Network(network), disposition)
        })
        .collect()
}

/// Classifies volume orphans by excluding desired identities.
pub fn classify_orphan_volumes(
    observed: impl Iterator<Item = ObservedVolume>,
    desired: &indexmap::IndexSet<VolumeIdentity>,
    policy: &OrphanPolicy,
) -> Vec<(OrphanResource, OrphanDisposition)> {
    observed
        .filter(|volume| {
            volume
                .volume_identity
                .as_ref()
                .is_some_and(|id| !desired.contains(id))
        })
        .map(|volume| {
            let disposition = if policy.remove_volumes {
                OrphanDisposition::Remove
            } else if policy.report {
                OrphanDisposition::Report
            } else {
                OrphanDisposition::Ignore
            };
            (OrphanResource::Volume(volume), disposition)
        })
        .collect()
}
