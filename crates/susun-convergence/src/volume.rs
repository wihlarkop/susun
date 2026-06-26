//! Volume preservation classification.

use susun_model::{CanonicalVolume, VolumeKind};

use crate::AnonymousVolumePolicy;

/// Replacement-time volume disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeDisposition {
    /// Preserve the volume/mount.
    Preserve,
    /// Recreate the volume.
    Recreate,
    /// Block because ownership is ambiguous.
    BlockAmbiguous,
}

/// Classifies how a mount should be handled during replacement.
pub fn classify_volume_for_replacement(
    volume: &CanonicalVolume,
    policy: AnonymousVolumePolicy,
) -> VolumeDisposition {
    match volume.kind {
        VolumeKind::Volume | VolumeKind::Bind => VolumeDisposition::Preserve,
        VolumeKind::Anonymous => match policy {
            AnonymousVolumePolicy::PreserveWhenTargetMatches => VolumeDisposition::Preserve,
            AnonymousVolumePolicy::Recreate => VolumeDisposition::Recreate,
            AnonymousVolumePolicy::RejectAmbiguous => VolumeDisposition::BlockAmbiguous,
        },
    }
}
