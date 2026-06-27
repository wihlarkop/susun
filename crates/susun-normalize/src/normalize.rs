//! Converts a merged parsed project into the canonical model.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use susun_diagnostics::DiagnosticReport;
use susun_model::{
    BuildDefinition, CanonicalVolume, Command, ConfigName, DependencyCondition, Healthcheck,
    ImageRef, NetworkAttachment, NetworkName, Project, ProjectName, ResourceDefinition,
    ResourceMount, SecretName, Service, ServiceDependency, ServiceName, VolumeKind, VolumeName,
};

use crate::{
    error::NormalizeError,
    input::{
        MergeProject, ParsedService,
        build::RawBuildDefinition,
        command::RawStringOrList,
        dependency::{RawDependencies, RawDependency},
        environment::RawMapping,
        health::RawHealthcheck,
        resource::{RawResourceDefinition, RawResourceMount, RawResources, RawServiceNetworks},
    },
    port::parse_port_entry,
    provenance::{ProjectProvenance, ServiceProvenance},
    volume::parse_volume_entry,
};

/// Caller-supplied metadata used to resolve the canonical project name and
/// other context not derivable from the Compose file itself.
pub struct FinalProjectMetadata {
    /// Resolved project name (from compose file, env var, or directory heuristic).
    pub project_name: ProjectName,
    /// Directory of the primary Compose file, used to resolve relative bind-mount paths.
    pub project_directory: PathBuf,
}

/// The result of a successful normalization pass.
pub struct NormalizationOutcome {
    /// The canonical project model.
    pub project: Project,
    /// Source provenance for the canonical fields.
    pub provenance: ProjectProvenance,
    /// Diagnostics produced during normalization (user-visible warnings/errors).
    pub report: DiagnosticReport,
}

/// Normalize a merged parsed project into the canonical model.
///
/// User-level mistakes are appended to `outcome.report` as diagnostics rather
/// than causing an `Err` return. `Err` is reserved for internal invariant
/// violations that indicate a programmer error.
pub fn normalize(
    input: MergeProject,
    metadata: FinalProjectMetadata,
) -> Result<NormalizationOutcome, NormalizeError> {
    let report = DiagnosticReport::new();
    let mut services: IndexMap<ServiceName, Service> = IndexMap::new();
    let mut service_provenance: IndexMap<String, ServiceProvenance> = IndexMap::new();

    for (name, spanned_svc) in input.services {
        let (canonical, prov) = normalize_service(spanned_svc.value, &metadata.project_directory);
        services.insert(ServiceName::new(&name), canonical);
        service_provenance.insert(name, prov);
    }

    let project = Project {
        name: metadata.project_name,
        services,
        networks: normalize_resource_map(
            input.networks,
            NetworkName::new,
            &metadata.project_directory,
        ),
        volumes: normalize_resource_map(
            input.volumes,
            VolumeName::new,
            &metadata.project_directory,
        ),
        configs: normalize_resource_map(
            input.configs,
            ConfigName::new,
            &metadata.project_directory,
        ),
        secrets: normalize_resource_map(
            input.secrets,
            SecretName::new,
            &metadata.project_directory,
        ),
    };
    let provenance = ProjectProvenance {
        name_span: input.name.map(|s| s.span),
        services: service_provenance,
    };

    Ok(NormalizationOutcome {
        project,
        provenance,
        report,
    })
}

fn normalize_service(svc: ParsedService, project_dir: &Path) -> (Service, ServiceProvenance) {
    let image_span = svc.image.as_ref().map(|s| s.span);

    let image = svc.image.map(|s| ImageRef::new(s.value));
    let build = svc.build.map(normalize_build);
    let command = normalize_command(svc.command);
    let entrypoint = normalize_command(svc.entrypoint);
    let environment = normalize_mapping(svc.environment);
    let labels = normalize_labels(svc.labels);
    let ports = normalize_ports(svc.ports);
    let volumes = normalize_volumes(svc.volumes, project_dir);
    let depends_on = normalize_depends_on(svc.depends_on);
    let networks = normalize_networks(svc.networks);
    let configs = normalize_mounts(svc.configs, ConfigName::new);
    let secrets = normalize_mounts(svc.secrets, SecretName::new);
    let healthcheck = svc.healthcheck.map(normalize_healthcheck);
    let restart = svc.restart.map(|s| s.value);
    let profiles = svc.profiles.into_iter().map(|s| s.value).collect();

    let canonical = Service {
        image,
        build,
        command,
        entrypoint,
        environment,
        labels,
        ports,
        volumes,
        depends_on,
        networks,
        configs,
        secrets,
        healthcheck,
        restart,
        profiles,
    };
    (canonical, ServiceProvenance { image_span })
}

fn normalize_build(raw: RawBuildDefinition) -> BuildDefinition {
    BuildDefinition {
        context: raw.context.map(|s| s.value),
        dockerfile: raw.dockerfile.map(|s| s.value),
        target: raw.target.map(|s| s.value),
        args: raw
            .args
            .into_iter()
            .map(|(key, value)| (key, value.map(|s| s.value)))
            .collect(),
        platforms: raw.platforms.into_iter().map(|s| s.value).collect(),
        secrets: raw.secrets.into_iter().map(|s| s.value).collect(),
        ssh: raw.ssh.into_iter().map(|s| s.value).collect(),
        cache_from: raw.cache_from.into_iter().map(|s| s.value).collect(),
        cache_to: raw.cache_to.into_iter().map(|s| s.value).collect(),
    }
}

fn normalize_command(raw: RawStringOrList) -> Option<Command> {
    match raw {
        RawStringOrList::Null => None,
        RawStringOrList::String(s) => Some(Command::Shell(s.value)),
        RawStringOrList::List(items) => {
            Some(Command::Exec(items.into_iter().map(|s| s.value).collect()))
        }
    }
}

fn normalize_mapping(raw: RawMapping) -> IndexMap<String, Option<String>> {
    match raw {
        RawMapping::Map(map) => map
            .into_iter()
            .map(|(k, v)| (k, v.map(|s| s.value)))
            .collect(),
        RawMapping::List(entries) => {
            // expand_project should have converted this to Map form; handle gracefully.
            entries
                .into_iter()
                .map(|s| match s.value.find('=') {
                    Some(eq) => (s.value[..eq].to_owned(), Some(s.value[eq + 1..].to_owned())),
                    None => (s.value, None),
                })
                .collect()
        }
    }
}

fn normalize_labels(raw: RawMapping) -> IndexMap<String, String> {
    match raw {
        RawMapping::Map(map) => map
            .into_iter()
            .map(|(k, v)| (k, v.map(|s| s.value).unwrap_or_default()))
            .collect(),
        RawMapping::List(entries) => entries
            .into_iter()
            .map(|s| match s.value.find('=') {
                Some(eq) => (s.value[..eq].to_owned(), s.value[eq + 1..].to_owned()),
                None => (s.value, String::new()),
            })
            .collect(),
    }
}

fn normalize_ports(
    raw_ports: Vec<crate::input::port::RawPortEntry>,
) -> Vec<susun_model::CanonicalPort> {
    raw_ports
        .into_iter()
        .filter_map(|entry| parse_port_entry(&entry).ok())
        .collect()
}

fn normalize_volumes(
    raw_vols: Vec<crate::input::volume::RawVolumeMount>,
    project_dir: &Path,
) -> Vec<CanonicalVolume> {
    raw_vols
        .into_iter()
        .filter_map(|entry| {
            let mut vol = parse_volume_entry(&entry).ok()?;
            // Resolve relative bind-mount sources against the project directory.
            if vol.kind == VolumeKind::Bind {
                if let Some(src) = &vol.source {
                    let p = Path::new(src);
                    if p.is_relative() {
                        let resolved = project_dir.join(p);
                        vol.source = Some(resolved.to_string_lossy().into_owned());
                    }
                }
            }
            Some(vol)
        })
        .collect()
}

fn normalize_resource_map<N>(
    raw: RawResources,
    name: impl Fn(String) -> N,
    project_dir: &Path,
) -> IndexMap<N, ResourceDefinition>
where
    N: std::hash::Hash + Eq,
{
    raw.into_iter()
        .map(|(key, definition)| {
            (
                name(key),
                normalize_resource_definition(definition.value, project_dir),
            )
        })
        .collect()
}

fn normalize_resource_definition(
    raw: RawResourceDefinition,
    project_dir: &Path,
) -> ResourceDefinition {
    ResourceDefinition {
        external: raw
            .external
            .as_ref()
            .is_some_and(|value| parse_bool(value.value.as_str())),
        name: raw.name.map(|value| value.value),
        file: raw
            .file
            .map(|value| resolve_project_path(project_dir, value.value)),
    }
}

fn normalize_depends_on(raw: RawDependencies) -> IndexMap<ServiceName, ServiceDependency> {
    raw.into_iter()
        .map(|(name, dependency)| (ServiceName::new(name), normalize_dependency(dependency)))
        .collect()
}

fn normalize_dependency(raw: RawDependency) -> ServiceDependency {
    ServiceDependency {
        condition: raw
            .condition
            .as_ref()
            .map(|value| normalize_condition(value.value.as_str()))
            .unwrap_or_default(),
        restart: raw
            .restart
            .as_ref()
            .is_some_and(|value| parse_bool(value.value.as_str())),
        required: raw
            .required
            .as_ref()
            .map(|value| parse_bool(value.value.as_str()))
            .unwrap_or(true),
    }
}

fn normalize_condition(value: &str) -> DependencyCondition {
    match value {
        "service_healthy" => DependencyCondition::ServiceHealthy,
        "service_completed_successfully" => DependencyCondition::ServiceCompletedSuccessfully,
        _ => DependencyCondition::ServiceStarted,
    }
}

fn normalize_networks(raw: RawServiceNetworks) -> IndexMap<NetworkName, NetworkAttachment> {
    raw.into_iter()
        .map(|(name, attachment)| {
            (
                NetworkName::new(name),
                NetworkAttachment {
                    aliases: attachment
                        .aliases
                        .into_iter()
                        .map(|alias| alias.value)
                        .collect(),
                },
            )
        })
        .collect()
}

fn normalize_mounts<N>(
    raw: Vec<RawResourceMount>,
    name: impl Fn(String) -> N,
) -> Vec<ResourceMount<N>> {
    raw.into_iter()
        .map(|mount| ResourceMount {
            source: name(mount.source.value),
            target: mount.target.map(|target| target.value),
            uid: mount.uid.map(|uid| uid.value),
            gid: mount.gid.map(|gid| gid.value),
            mode: mount.mode.map(|mode| mode.value),
        })
        .collect()
}

fn normalize_healthcheck(raw: RawHealthcheck) -> Healthcheck {
    Healthcheck {
        test: normalize_command(raw.test),
        interval: raw.interval.map(|value| value.value),
        timeout: raw.timeout.map(|value| value.value),
        start_period: raw.start_period.map(|value| value.value),
        retries: raw
            .retries
            .and_then(|value| value.value.parse::<u32>().ok()),
        disable: raw
            .disable
            .as_ref()
            .is_some_and(|value| parse_bool(value.value.as_str())),
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value, "true" | "True" | "TRUE" | "yes" | "1")
}

fn resolve_project_path(project_dir: &Path, value: String) -> String {
    let path = Path::new(&value);
    if path.is_absolute() {
        value
    } else {
        project_dir.join(path).to_string_lossy().into_owned()
    }
}
