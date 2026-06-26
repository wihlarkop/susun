//! Image identity change classification.

use susun_engine::ObservedImageRef;
use susun_model::ImageRef;

use crate::ImageChangePolicy;

/// Desired image identity used for convergence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesiredImageIdentity {
    /// Desired reference.
    pub reference: Option<ImageRef>,
    /// Desired digest when available.
    pub digest: Option<String>,
}

/// Image comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDifference {
    /// Image identity is compatible.
    Unchanged,
    /// Image identity changed.
    Changed,
    /// Not enough metadata is available for this policy.
    Unknown,
}

/// Compares desired and observed image identity according to policy.
pub fn classify_image_difference(
    desired: &DesiredImageIdentity,
    observed: &ObservedImageRef,
    policy: ImageChangePolicy,
) -> ImageDifference {
    match policy {
        ImageChangePolicy::ReferenceOnly => compare_reference(desired.reference.as_ref(), observed),
        ImageChangePolicy::DigestWhenAvailable => {
            if let Some(digest) = &desired.digest {
                compare_digest(digest, observed)
            } else {
                compare_reference(desired.reference.as_ref(), observed)
            }
        }
        ImageChangePolicy::AlwaysRefreshAccordingToPullPolicy => ImageDifference::Changed,
    }
}

fn compare_reference(desired: Option<&ImageRef>, observed: &ObservedImageRef) -> ImageDifference {
    match (desired, observed) {
        (Some(desired), ObservedImageRef::Reference(observed)) if desired == observed => {
            ImageDifference::Unchanged
        }
        (Some(_), ObservedImageRef::Reference(_)) => ImageDifference::Changed,
        (None, _) => ImageDifference::Unknown,
        (_, ObservedImageRef::Unknown) => ImageDifference::Unknown,
        (_, ObservedImageRef::Id(_)) => ImageDifference::Unknown,
    }
}

fn compare_digest(desired: &str, observed: &ObservedImageRef) -> ImageDifference {
    match observed {
        ObservedImageRef::Reference(reference) if reference.as_str().contains(desired) => {
            ImageDifference::Unchanged
        }
        ObservedImageRef::Reference(_) => ImageDifference::Changed,
        ObservedImageRef::Id(id) if id.as_str() == desired => ImageDifference::Unchanged,
        ObservedImageRef::Id(_) => ImageDifference::Changed,
        ObservedImageRef::Unknown => ImageDifference::Unknown,
    }
}
