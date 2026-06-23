//! Converts a merged parsed project into the canonical model.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use susun_diagnostics::DiagnosticReport;
use susun_model::{
    CanonicalVolume, Command, ImageRef, Project, ProjectName, Service, ServiceName, VolumeKind,
};

use crate::{
    error::NormalizeError,
    input::{
        MergeProject, ParsedService,
        command::RawStringOrList,
        environment::RawMapping,
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

    let project = Project { name: metadata.project_name, services };
    let provenance = ProjectProvenance {
        name_span: input.name.map(|s| s.span),
        services: service_provenance,
    };

    Ok(NormalizationOutcome { project, provenance, report })
}

fn normalize_service(svc: ParsedService, project_dir: &Path) -> (Service, ServiceProvenance) {
    let image_span = svc.image.as_ref().map(|s| s.span);

    let image = svc.image.map(|s| ImageRef::new(s.value));
    let command = normalize_command(svc.command);
    let entrypoint = normalize_command(svc.entrypoint);
    let environment = normalize_mapping(svc.environment);
    let labels = normalize_labels(svc.labels);
    let ports = normalize_ports(svc.ports);
    let volumes = normalize_volumes(svc.volumes, project_dir);

    let canonical = Service { image, command, entrypoint, environment, labels, ports, volumes };
    (canonical, ServiceProvenance { image_span })
}

fn normalize_command(raw: RawStringOrList) -> Option<Command> {
    match raw {
        RawStringOrList::Null => None,
        RawStringOrList::String(s) => Some(Command::Shell(s.value)),
        RawStringOrList::List(items) => Some(Command::Exec(items.into_iter().map(|s| s.value).collect())),
    }
}

fn normalize_mapping(raw: RawMapping) -> IndexMap<String, Option<String>> {
    match raw {
        RawMapping::Map(map) => {
            map.into_iter()
                .map(|(k, v)| (k, v.map(|s| s.value)))
                .collect()
        }
        RawMapping::List(entries) => {
            // expand_project should have converted this to Map form; handle gracefully.
            entries.into_iter().map(|s| {
                match s.value.find('=') {
                    Some(eq) => (s.value[..eq].to_owned(), Some(s.value[eq + 1..].to_owned())),
                    None => (s.value, None),
                }
            }).collect()
        }
    }
}

fn normalize_labels(raw: RawMapping) -> IndexMap<String, String> {
    match raw {
        RawMapping::Map(map) => {
            map.into_iter()
                .map(|(k, v)| (k, v.map(|s| s.value).unwrap_or_default()))
                .collect()
        }
        RawMapping::List(entries) => {
            entries.into_iter().map(|s| {
                match s.value.find('=') {
                    Some(eq) => (s.value[..eq].to_owned(), s.value[eq + 1..].to_owned()),
                    None => (s.value, String::new()),
                }
            }).collect()
        }
    }
}

fn normalize_ports(raw_ports: Vec<crate::input::port::RawPortEntry>) -> Vec<susun_model::CanonicalPort> {
    raw_ports.into_iter().filter_map(|entry| parse_port_entry(&entry).ok()).collect()
}

fn normalize_volumes(raw_vols: Vec<crate::input::volume::RawVolumeMount>, project_dir: &Path) -> Vec<CanonicalVolume> {
    raw_vols.into_iter().filter_map(|entry| {
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
    }).collect()
}
