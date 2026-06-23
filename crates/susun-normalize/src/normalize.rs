//! Converts a merged parsed project into the canonical model.

use indexmap::IndexMap;
use susun_diagnostics::DiagnosticReport;
use susun_model::{ImageRef, Project, ProjectName, Service, ServiceName};

use crate::{
    error::NormalizeError,
    input::{MergeProject, ParsedService},
    provenance::{ProjectProvenance, ServiceProvenance},
};

/// Caller-supplied metadata used to resolve the canonical project name and
/// other context not derivable from the Compose file itself.
pub struct FinalProjectMetadata {
    /// Resolved project name (from compose file, env var, or directory heuristic).
    pub project_name: ProjectName,
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
        let (canonical, prov) = normalize_service(spanned_svc.value);
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

fn normalize_service(svc: ParsedService) -> (Service, ServiceProvenance) {
    let image_span = svc.image.as_ref().map(|s| s.span);
    let canonical = Service {
        image: svc.image.map(|s| ImageRef::new(s.value)),
    };
    (canonical, ServiceProvenance { image_span })
}
