//! Capability preflight diagnostics.

use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::{MountType, SupportLevel};
use susun_model::{DependencyCondition, VolumeKind};

use crate::{DependencyWaitPolicy, PlanningInput, UpPlanOptions};

const UNSUPPORTED_MOUNT: &str = "SUS-CAP-001";
const UNSUPPORTED_HEALTH: &str = "SUS-CAP-002";
const UNSUPPORTED_ALIAS: &str = "SUS-CAP-003";
const MISSING_IMAGE: &str = "SUS-CAP-004";

/// Runs capability checks for `up` planning.
pub fn check_up_capabilities(
    input: &PlanningInput<'_>,
    options: UpPlanOptions,
) -> DiagnosticReport {
    let mut report = DiagnosticReport::new();

    for service_name in &input.selection.active_services {
        let Some(service) = input.project.services.get(service_name) else {
            continue;
        };

        if service.image.is_none() {
            report.push(Diagnostic::new(
                MISSING_IMAGE,
                Severity::Error,
                format!(
                    "service '{}' has no prebuilt image available for Phase 2 planning",
                    service_name.as_str()
                ),
            ));
        }

        for volume in &service.volumes {
            let mount_type = mount_type_for(volume.kind);
            if !input.capabilities.supports_mount(mount_type) {
                report.push(Diagnostic::new(
                    UNSUPPORTED_MOUNT,
                    Severity::Error,
                    format!(
                        "service '{}' requires unsupported {:?} mount",
                        service_name.as_str(),
                        mount_type
                    ),
                ));
            }
        }

        for (network_name, attachment) in &service.networks {
            if !attachment.aliases.is_empty()
                && !input.capabilities.supports_network_aliases.is_supported()
            {
                report.push(Diagnostic::new(
                    UNSUPPORTED_ALIAS,
                    Severity::Error,
                    format!(
                        "service '{}' uses aliases on network '{}' but aliases are not supported",
                        service_name.as_str(),
                        network_name.as_str()
                    ),
                ));
            }
        }

        if service.healthcheck.is_some() && !input.capabilities.supports_health.is_supported() {
            report.push(Diagnostic::new(
                UNSUPPORTED_HEALTH,
                Severity::Error,
                format!(
                    "service '{}' defines a healthcheck but health is not supported",
                    service_name.as_str()
                ),
            ));
        }

        for (dependency_name, dependency) in &service.depends_on {
            if dependency.condition == DependencyCondition::ServiceHealthy
                && options.dependency_wait_policy != DependencyWaitPolicy::DegradeToStartOrder
                && input.capabilities.supports_health != SupportLevel::Supported
            {
                report.push(Diagnostic::new(
                    UNSUPPORTED_HEALTH,
                    Severity::Error,
                    format!(
                        "service '{}' waits for '{}' to become healthy but health waits are not supported",
                        service_name.as_str(),
                        dependency_name.as_str()
                    ),
                ));
            }
        }
    }

    report
}

fn mount_type_for(kind: VolumeKind) -> MountType {
    match kind {
        VolumeKind::Volume => MountType::Volume,
        VolumeKind::Bind => MountType::Bind,
        VolumeKind::Anonymous => MountType::Anonymous,
    }
}
