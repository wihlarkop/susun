//! Semantic validation for normalized Compose projects.

use std::{collections::HashSet, path::Path};

use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_model::{
    DependencyCondition, Project, Protocol, PublishedPort, ResourceDefinition, ServiceName,
};
use susun_normalize::selection::ProjectSelection;

const UNKNOWN_SERVICE: &str = "SUS-SEM-001";
const DUPLICATE_PORT: &str = "SUS-SEM-002";
const UNKNOWN_VOLUME: &str = "SUS-SEM-003";
const UNKNOWN_NETWORK: &str = "SUS-SEM-004";
const UNKNOWN_CONFIG_SECRET: &str = "SUS-SEM-005";
const HEALTH_WITHOUT_CHECK: &str = "SUS-SEM-006";
const INVALID_CONFIG_SECRET: &str = "SUS-SEM-007";

/// Semantic validation result.
pub struct ValidationOutcome {
    /// Diagnostics produced by validation passes.
    pub report: DiagnosticReport,
}

/// Validate active services in a normalized project.
pub fn validate(project: &Project, selection: &ProjectSelection) -> ValidationOutcome {
    let mut report = DiagnosticReport::new();
    validate_references(project, selection, &mut report);
    validate_config_secret_definitions(project, &mut report);
    validate_ports(project, selection, &mut report);
    validate_health_dependencies(project, selection, &mut report);
    ValidationOutcome { report }
}

fn validate_config_secret_definitions(project: &Project, report: &mut DiagnosticReport) {
    for (name, definition) in &project.configs {
        validate_config_secret_definition("config", name.as_str(), definition, report);
    }

    for (name, definition) in &project.secrets {
        validate_config_secret_definition("secret", name.as_str(), definition, report);
    }
}

fn validate_config_secret_definition(
    kind: &str,
    name: &str,
    definition: &ResourceDefinition,
    report: &mut DiagnosticReport,
) {
    if definition.external && definition.file.is_some() {
        report.push(Diagnostic::new(
            INVALID_CONFIG_SECRET,
            Severity::Error,
            format!("{kind} `{name}` cannot be both external and file-backed"),
        ));
    }

    if let Some(file) = &definition.file
        && !Path::new(file).is_file()
    {
        report.push(Diagnostic::new(
            INVALID_CONFIG_SECRET,
            Severity::Error,
            format!("{kind} `{name}` file `{file}` does not exist"),
        ));
    }
}

fn validate_references(
    project: &Project,
    selection: &ProjectSelection,
    report: &mut DiagnosticReport,
) {
    for (service_name, service) in active_services(project, selection) {
        for dependency in service.depends_on.keys() {
            if !project.services.contains_key(dependency) {
                report.push(Diagnostic::new(
                    UNKNOWN_SERVICE,
                    Severity::Error,
                    format!(
                        "service `{}` depends on unknown service `{}`",
                        service_name, dependency
                    ),
                ));
            }
        }

        for network in service.networks.keys() {
            if !project.networks.contains_key(network) {
                report.push(Diagnostic::new(
                    UNKNOWN_NETWORK,
                    Severity::Error,
                    format!(
                        "service `{}` references unknown network `{}`",
                        service_name, network
                    ),
                ));
            }
        }

        for volume in &service.volumes {
            if volume.kind == susun_model::VolumeKind::Volume {
                if let Some(source) = &volume.source {
                    if !project
                        .volumes
                        .contains_key(&susun_model::VolumeName::new(source))
                    {
                        report.push(Diagnostic::new(
                            UNKNOWN_VOLUME,
                            Severity::Error,
                            format!(
                                "service `{}` references unknown volume `{}`",
                                service_name, source
                            ),
                        ));
                    }
                }
            }
        }

        for mount in &service.configs {
            if !project.configs.contains_key(&mount.source) {
                report.push(Diagnostic::new(
                    UNKNOWN_CONFIG_SECRET,
                    Severity::Error,
                    format!(
                        "service `{}` references unknown config `{}`",
                        service_name, mount.source
                    ),
                ));
            }
        }

        for mount in &service.secrets {
            if !project.secrets.contains_key(&mount.source) {
                report.push(Diagnostic::new(
                    UNKNOWN_CONFIG_SECRET,
                    Severity::Error,
                    format!(
                        "service `{}` references unknown secret `{}`",
                        service_name, mount.source
                    ),
                ));
            }
        }
    }
}

fn validate_ports(project: &Project, selection: &ProjectSelection, report: &mut DiagnosticReport) {
    let mut seen = HashSet::new();
    for (service_name, service) in active_services(project, selection) {
        for port in &service.ports {
            let Some(published) = &port.published else {
                continue;
            };
            let key = port_key(port.host_ip.as_deref(), published, port.protocol);
            if !seen.insert(key.clone()) {
                report.push(Diagnostic::new(
                    DUPLICATE_PORT,
                    Severity::Error,
                    format!(
                        "service `{}` publishes duplicate host port `{}`",
                        service_name, key
                    ),
                ));
            }
        }
    }
}

fn validate_health_dependencies(
    project: &Project,
    selection: &ProjectSelection,
    report: &mut DiagnosticReport,
) {
    for (service_name, service) in active_services(project, selection) {
        for (dependency_name, dependency) in &service.depends_on {
            if dependency.condition != DependencyCondition::ServiceHealthy {
                continue;
            }
            let Some(dependency_service) = project.services.get(dependency_name) else {
                continue;
            };
            if dependency_service.healthcheck.is_none() {
                report.push(Diagnostic::new(
                    HEALTH_WITHOUT_CHECK,
                    Severity::Error,
                    format!(
                        "service `{}` requires `{}` to be healthy, but `{}` has no healthcheck",
                        service_name, dependency_name, dependency_name
                    ),
                ));
            }
        }
    }
}

fn active_services<'a>(
    project: &'a Project,
    selection: &'a ProjectSelection,
) -> impl Iterator<Item = (&'a ServiceName, &'a susun_model::Service)> {
    project
        .services
        .iter()
        .filter(|(name, _)| selection.active_services.contains(*name))
}

fn port_key(host_ip: Option<&str>, published: &PublishedPort, protocol: Protocol) -> String {
    let published = match published {
        PublishedPort::Single(port) => port.to_string(),
        PublishedPort::Range { start, end } => format!("{start}-{end}"),
    };
    format!("{}:{published}/{protocol:?}", host_ip.unwrap_or(""))
}
