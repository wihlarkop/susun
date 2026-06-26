//! Convergence diagnostic catalog.

use susun_diagnostics::{Diagnostic, Severity};
use susun_engine::{ResourceName, ServiceInstanceId};

use crate::{InstanceDifference, OwnershipConflict};

/// Duplicate observed resource ownership claim.
pub const SUS_OWN_DUPLICATE_CLAIM: &str = "SUS-OWN-001";
/// Foreign resource collides with a desired runtime name.
pub const SUS_OWN_FOREIGN_NAME: &str = "SUS-OWN-002";
/// Desired service instance is missing.
pub const SUS_DIFF_MISSING: &str = "SUS-DIFF-001";
/// Desired service instance configuration changed.
pub const SUS_DIFF_CONFIG_CHANGED: &str = "SUS-DIFF-002";
/// Observed state cannot be safely interpreted.
pub const SUS_DIFF_UNSUPPORTED_STATE: &str = "SUS-DIFF-003";
/// Observed fingerprint version is unsupported.
pub const SUS_DIFF_UNSUPPORTED_VERSION: &str = "SUS-DIFF-004";
/// Fingerprint version is unsupported.
pub const SUS_FP_UNSUPPORTED_VERSION: &str = "SUS-FP-001";

/// Creates a diagnostic for duplicate ownership claims.
pub fn ownership_conflict_diagnostic(conflict: &OwnershipConflict) -> Diagnostic {
    Diagnostic::new(
        SUS_OWN_DUPLICATE_CLAIM,
        Severity::Error,
        format!(
            "multiple observed containers claim service instance {}",
            display_instance(&conflict.instance)
        ),
    )
    .with_help("Resolve duplicate Susun ownership labels before destructive convergence.")
}

/// Creates a diagnostic for foreign runtime-name conflicts.
pub fn foreign_name_conflict_diagnostic(name: &ResourceName) -> Diagnostic {
    Diagnostic::new(
        SUS_OWN_FOREIGN_NAME,
        Severity::Error,
        format!("foreign resource uses desired runtime name `{name}`"),
    )
    .with_help("Rename or remove the foreign resource, or choose a different project identity.")
}

/// Creates a diagnostic for an unsupported fingerprint version.
pub fn fingerprint_version_diagnostic(version: u16) -> Diagnostic {
    Diagnostic::new(
        SUS_FP_UNSUPPORTED_VERSION,
        Severity::Error,
        format!("observed fingerprint version {version} is not supported"),
    )
    .with_help("Recreate the resource or run a Susun version that can migrate this fingerprint.")
}

/// Creates a diagnostic for a classified convergence difference when it must block.
pub fn convergence_diagnostic_for_difference(
    instance: &ServiceInstanceId,
    difference: &InstanceDifference,
) -> Option<Diagnostic> {
    match difference {
        InstanceDifference::Missing => Some(Diagnostic::new(
            SUS_DIFF_MISSING,
            Severity::Note,
            format!("service instance {} is missing", display_instance(instance)),
        )),
        InstanceDifference::ConfigurationChanged { fields } => Some(Diagnostic::new(
            SUS_DIFF_CONFIG_CHANGED,
            Severity::Warning,
            format!(
                "service instance {} changed configuration fields: {}",
                display_instance(instance),
                fields
                    .iter()
                    .map(|field| field.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
        InstanceDifference::OwnershipAmbiguous => Some(Diagnostic::new(
            SUS_OWN_DUPLICATE_CLAIM,
            Severity::Error,
            format!(
                "service instance {} has ambiguous ownership",
                display_instance(instance)
            ),
        )),
        InstanceDifference::UnsupportedObservedState => Some(Diagnostic::new(
            SUS_DIFF_UNSUPPORTED_STATE,
            Severity::Error,
            format!(
                "service instance {} has unsupported observed state",
                display_instance(instance)
            ),
        )),
        InstanceDifference::Unchanged
        | InstanceDifference::StoppedButCompatible
        | InstanceDifference::ImageChanged
        | InstanceDifference::RuntimeStateDrift { .. } => None,
    }
}

fn display_instance(instance: &ServiceInstanceId) -> String {
    format!(
        "{}/{}[{}]",
        instance.project.as_str(),
        instance.service.as_str(),
        instance.replica.as_u32()
    )
}
