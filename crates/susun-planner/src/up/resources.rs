//! Prerequisite resource planning for `up`.

use indexmap::{IndexMap, IndexSet};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_engine::{NetworkIdentity, VolumeIdentity};
use susun_model::{ImageRef, VolumeKind, VolumeName};

use crate::{
    ActionExplanation, ActionId, ActionReason, ActionSafety, CreateNetworkAction,
    CreateVolumeAction, ImageAcquisitionPolicy, NamingPolicy, NoOpAction, PlanAction,
    PlanActionNode, PlanError, PlannedOperation, PlanningInput, PullImageAction, UpPlanOptions,
};

use super::insert_action;

const FOREIGN_RESOURCE: &str = "SUS-PLAN-001";
const REQUIRED_IMAGE_MISSING: &str = "SUS-PLAN-003";

/// Action IDs for prerequisite resources.
#[derive(Debug, Clone, Default)]
pub struct UpResourceActions {
    /// Network prerequisite action by canonical network name.
    pub networks: IndexMap<String, ActionId>,
    /// Volume prerequisite action by canonical volume name.
    pub volumes: IndexMap<String, ActionId>,
    /// Image prerequisite action by image reference.
    pub images: IndexMap<String, ActionId>,
}

pub(crate) fn plan_prerequisite_resources(
    input: &PlanningInput<'_>,
    options: UpPlanOptions,
    naming: &dyn NamingPolicy,
    actions: &mut IndexMap<ActionId, PlanActionNode>,
    diagnostics: &mut DiagnosticReport,
) -> Result<UpResourceActions, PlanError> {
    let mut planned = UpResourceActions::default();

    for network_name in input.project.networks.keys() {
        let identity =
            NetworkIdentity::new(input.identity.working_set.clone(), network_name.clone());
        let runtime_name =
            naming
                .network_name(&identity)
                .map_err(|err| PlanError::InvariantViolation {
                    detail: err.to_string(),
                })?;
        if network_exists_owned(input, runtime_name.as_str()) {
            let action = PlanAction::NoOp(NoOpAction {
                resource: format!("network:{}", network_name.as_str()),
                description: format!("network '{}' already exists", runtime_name.as_str()),
            });
            let id = make_action_id(input, &action, "0");
            let node = node(
                id,
                action,
                ActionReason::ExistingResourceAccepted,
                "existing owned network accepted",
                ActionSafety::Safe,
            );
            planned.networks.insert(
                network_name.as_str().to_owned(),
                insert_action(actions, node)?,
            );
        } else if network_name_conflicts(input, runtime_name.as_str()) {
            diagnostics.push(foreign_resource_diagnostic(
                "network",
                runtime_name.as_str(),
            ));
        } else {
            let action = PlanAction::CreateNetwork(CreateNetworkAction {
                identity,
                name: runtime_name,
            });
            let id = make_action_id(input, &action, "0");
            let node = node(
                id,
                action,
                ActionReason::ResourceMissing,
                "declared network is missing",
                ActionSafety::Safe,
            );
            planned.networks.insert(
                network_name.as_str().to_owned(),
                insert_action(actions, node)?,
            );
        }
    }

    for volume_name in required_named_volumes(input) {
        let identity = VolumeIdentity::new(input.identity.working_set.clone(), volume_name.clone());
        let runtime_name =
            naming
                .volume_name(&identity)
                .map_err(|err| PlanError::InvariantViolation {
                    detail: err.to_string(),
                })?;
        if volume_exists_owned(input, runtime_name.as_str()) {
            let action = PlanAction::NoOp(NoOpAction {
                resource: format!("volume:{}", volume_name.as_str()),
                description: format!("volume '{}' already exists", runtime_name.as_str()),
            });
            let id = make_action_id(input, &action, "0");
            let node = node(
                id,
                action,
                ActionReason::ExistingResourceAccepted,
                "existing owned volume accepted",
                ActionSafety::Safe,
            );
            planned.volumes.insert(
                volume_name.as_str().to_owned(),
                insert_action(actions, node)?,
            );
        } else if volume_name_conflicts(input, runtime_name.as_str()) {
            diagnostics.push(foreign_resource_diagnostic("volume", runtime_name.as_str()));
        } else {
            let action = PlanAction::CreateVolume(CreateVolumeAction {
                identity,
                name: runtime_name,
            });
            let id = make_action_id(input, &action, "0");
            let node = node(
                id,
                action,
                ActionReason::ResourceMissing,
                "named volume is missing",
                ActionSafety::Safe,
            );
            planned.volumes.insert(
                volume_name.as_str().to_owned(),
                insert_action(actions, node)?,
            );
        }
    }

    if options.image_policy != ImageAcquisitionPolicy::NeverPull {
        for image in required_images(input) {
            if image_exists(input, &image) {
                continue;
            }
            if options.image_policy == ImageAcquisitionPolicy::RequirePresent {
                diagnostics.push(Diagnostic::new(
                    REQUIRED_IMAGE_MISSING,
                    Severity::Error,
                    format!("required image '{}' is missing", image.as_str()),
                ));
                continue;
            }

            let action = PlanAction::PullImage(PullImageAction {
                image: image.clone(),
            });
            let id = make_action_id(input, &action, "0");
            let node = node(
                id,
                action,
                ActionReason::ImageUnavailableLocally,
                "image is not present in the engine snapshot",
                ActionSafety::Safe,
            );
            planned
                .images
                .insert(image.as_str().to_owned(), insert_action(actions, node)?);
        }
    }

    Ok(planned)
}

pub(crate) fn make_action_id(
    input: &PlanningInput<'_>,
    action: &PlanAction,
    discriminator: &str,
) -> ActionId {
    ActionId::from_parts(&[
        "1",
        input.identity.working_set.as_str(),
        PlannedOperation::Up.as_str(),
        &action.resource_key(),
        action.kind(),
        discriminator,
    ])
}

pub(crate) fn node(
    id: ActionId,
    action: PlanAction,
    reason: ActionReason,
    message: &str,
    safety: ActionSafety,
) -> PlanActionNode {
    PlanActionNode {
        id,
        action,
        dependencies: IndexSet::new(),
        reason: ActionExplanation::new(reason, message),
        safety,
    }
}

fn required_named_volumes(input: &PlanningInput<'_>) -> Vec<VolumeName> {
    let mut names = IndexSet::new();
    for name in input.project.volumes.keys() {
        names.insert(name.clone());
    }

    for service_name in &input.selection.active_services {
        let Some(service) = input.project.services.get(service_name) else {
            continue;
        };
        for volume in &service.volumes {
            if volume.kind != VolumeKind::Volume {
                continue;
            }
            if let Some(source) = &volume.source {
                names.insert(VolumeName::new(source.clone()));
            }
        }
    }

    names.into_iter().collect()
}

fn required_images(input: &PlanningInput<'_>) -> Vec<ImageRef> {
    let mut images = IndexSet::new();
    for service_name in &input.selection.active_services {
        let Some(service) = input.project.services.get(service_name) else {
            continue;
        };
        if let Some(image) = &service.image {
            images.insert(image.clone());
        }
    }
    images.into_iter().collect()
}

fn image_exists(input: &PlanningInput<'_>, image: &ImageRef) -> bool {
    input.snapshot.images.values().any(|observed| {
        observed
            .references
            .iter()
            .any(|reference| reference == image)
    })
}

fn network_exists_owned(input: &PlanningInput<'_>, name: &str) -> bool {
    input.snapshot.networks.values().any(|network| {
        network.name.as_str() == name
            && network.project_identity.as_ref() == Some(&input.identity.working_set)
    })
}

fn network_name_conflicts(input: &PlanningInput<'_>, name: &str) -> bool {
    input.snapshot.networks.values().any(|network| {
        network.name.as_str() == name
            && network.project_identity.as_ref() != Some(&input.identity.working_set)
    })
}

fn volume_exists_owned(input: &PlanningInput<'_>, name: &str) -> bool {
    input.snapshot.volumes.values().any(|volume| {
        volume.name.as_str() == name
            && volume.project_identity.as_ref() == Some(&input.identity.working_set)
    })
}

fn volume_name_conflicts(input: &PlanningInput<'_>, name: &str) -> bool {
    input.snapshot.volumes.values().any(|volume| {
        volume.name.as_str() == name
            && volume.project_identity.as_ref() != Some(&input.identity.working_set)
    })
}

fn foreign_resource_diagnostic(kind: &str, name: &str) -> Diagnostic {
    Diagnostic::new(
        FOREIGN_RESOURCE,
        Severity::Error,
        format!("foreign {kind} '{name}' occupies a required runtime name"),
    )
}
