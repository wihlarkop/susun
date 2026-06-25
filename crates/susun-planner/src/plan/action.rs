//! Execution plan action model.

use std::time::SystemTime;

use indexmap::{IndexMap, IndexSet};
use susun_diagnostics::DiagnosticReport;
use susun_engine::{
    NetworkIdentity, ProjectIdentity, ResourceName, ServiceInstanceId, VolumeIdentity,
};
use susun_model::ImageRef;

use crate::{
    ActionExplanation, ActionId, ActionSafety, PlanId, PlanSchemaVersion, StableIdBuilder,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Planned operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PlannedOperation {
    /// Bring selected services up.
    Up,
    /// Tear selected services down.
    Down,
}

impl PlannedOperation {
    /// Returns the stable operation key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

/// Immutable execution plan.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExecutionPlan {
    /// Serialized plan schema version.
    pub schema_version: PlanSchemaVersion,
    /// Stable plan ID.
    pub plan_id: PlanId,
    /// Project identity.
    pub project: ProjectIdentity,
    /// Planned operation.
    pub operation: PlannedOperation,
    /// Optional observational creation time.
    pub created_at: Option<SystemTime>,
    /// Actions keyed by stable action ID.
    pub actions: IndexMap<ActionId, PlanActionNode>,
    /// Plan summary.
    pub summary: PlanSummary,
    /// Diagnostics associated with planning.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub diagnostics: DiagnosticReport,
}

impl ExecutionPlan {
    /// Creates an execution plan and computes a deterministic plan ID.
    pub fn new(
        project: ProjectIdentity,
        operation: PlannedOperation,
        actions: IndexMap<ActionId, PlanActionNode>,
        diagnostics: DiagnosticReport,
    ) -> Self {
        let schema_version = PlanSchemaVersion::CURRENT;
        let plan_id = stable_plan_id(schema_version, &project, operation, &actions);
        let summary = PlanSummary::from_actions(&actions);

        Self {
            schema_version,
            plan_id,
            project,
            operation,
            created_at: None,
            actions,
            summary,
            diagnostics,
        }
    }
}

/// Planned action node with dependencies and explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PlanActionNode {
    /// Stable action ID.
    pub id: ActionId,
    /// Action payload.
    pub action: PlanAction,
    /// Action IDs that must complete first.
    pub dependencies: IndexSet<ActionId>,
    /// Explanation for why this action exists.
    pub reason: ActionExplanation,
    /// Safety classification.
    pub safety: ActionSafety,
}

/// Planned action payload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", content = "payload", rename_all = "snake_case")
)]
pub enum PlanAction {
    /// Pull an image.
    PullImage(PullImageAction),
    /// Create a network.
    CreateNetwork(CreateNetworkAction),
    /// Create a volume.
    CreateVolume(CreateVolumeAction),
    /// Create a container.
    CreateContainer(CreateContainerAction),
    /// Start a container.
    StartContainer(StartContainerAction),
    /// Wait for a dependency condition.
    WaitForDependency(WaitForDependencyAction),
    /// Stop a container.
    StopContainer(StopContainerAction),
    /// Remove a container.
    RemoveContainer(RemoveContainerAction),
    /// Remove a network.
    RemoveNetwork(RemoveNetworkAction),
    /// Remove a volume.
    RemoveVolume(RemoveVolumeAction),
    /// Record that no mutation is required.
    NoOp(NoOpAction),
}

impl PlanAction {
    /// Stable action kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::PullImage(_) => "pull_image",
            Self::CreateNetwork(_) => "create_network",
            Self::CreateVolume(_) => "create_volume",
            Self::CreateContainer(_) => "create_container",
            Self::StartContainer(_) => "start_container",
            Self::WaitForDependency(_) => "wait_for_dependency",
            Self::StopContainer(_) => "stop_container",
            Self::RemoveContainer(_) => "remove_container",
            Self::RemoveNetwork(_) => "remove_network",
            Self::RemoveVolume(_) => "remove_volume",
            Self::NoOp(_) => "no_op",
        }
    }

    /// Stable resource key for ID generation.
    pub fn resource_key(&self) -> String {
        match self {
            Self::PullImage(action) => format!("image:{}", action.image.as_str()),
            Self::CreateNetwork(action) => format!("network:{}", action.identity.network.as_str()),
            Self::CreateVolume(action) => format!("volume:{}", action.identity.volume.as_str()),
            Self::CreateContainer(action) => {
                format!("service:{}", action.identity.service.as_str())
            }
            Self::StartContainer(action) => format!("service:{}", action.identity.service.as_str()),
            Self::WaitForDependency(action) => {
                format!("dependency:{}", action.dependent.service.as_str())
            }
            Self::StopContainer(action) => format!("service:{}", action.identity.service.as_str()),
            Self::RemoveContainer(action) => {
                format!("service:{}", action.identity.service.as_str())
            }
            Self::RemoveNetwork(action) => format!("network:{}", action.identity.network.as_str()),
            Self::RemoveVolume(action) => format!("volume:{}", action.identity.volume.as_str()),
            Self::NoOp(action) => action.resource.clone(),
        }
    }
}

/// Pull-image action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PullImageAction {
    /// Image to pull.
    pub image: ImageRef,
}

/// Create-network action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CreateNetworkAction {
    /// Network identity.
    pub identity: NetworkIdentity,
    /// Runtime name.
    pub name: ResourceName,
}

/// Create-volume action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CreateVolumeAction {
    /// Volume identity.
    pub identity: VolumeIdentity,
    /// Runtime name.
    pub name: ResourceName,
}

/// Create-container action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CreateContainerAction {
    /// Service instance identity.
    pub identity: ServiceInstanceId,
    /// Runtime container name.
    pub name: ResourceName,
    /// Image to run.
    pub image: Option<ImageRef>,
}

/// Start-container action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StartContainerAction {
    /// Service instance identity.
    pub identity: ServiceInstanceId,
}

/// Wait-for-dependency action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct WaitForDependencyAction {
    /// Dependent service instance.
    pub dependent: ServiceInstanceId,
    /// Dependency service instance.
    pub dependency: ServiceInstanceId,
    /// Dependency condition key.
    pub condition: String,
}

/// Stop-container action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StopContainerAction {
    /// Service instance identity.
    pub identity: ServiceInstanceId,
}

/// Remove-container action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RemoveContainerAction {
    /// Service instance identity.
    pub identity: ServiceInstanceId,
}

/// Remove-network action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RemoveNetworkAction {
    /// Network identity.
    pub identity: NetworkIdentity,
}

/// Remove-volume action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RemoveVolumeAction {
    /// Volume identity.
    pub identity: VolumeIdentity,
}

/// No-op action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoOpAction {
    /// Stable resource key.
    pub resource: String,
    /// Human-readable description.
    pub description: String,
}

/// Plan summary counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PlanSummary {
    /// Total actions.
    pub total_actions: usize,
    /// Safe actions.
    pub safe_actions: usize,
    /// Caution actions.
    pub caution_actions: usize,
    /// Destructive actions.
    pub destructive_actions: usize,
}

impl PlanSummary {
    /// Computes summary counts from actions.
    pub fn from_actions(actions: &IndexMap<ActionId, PlanActionNode>) -> Self {
        let mut summary = Self {
            total_actions: actions.len(),
            ..Self::default()
        };

        for action in actions.values() {
            match action.safety {
                ActionSafety::Safe => summary.safe_actions += 1,
                ActionSafety::Caution => summary.caution_actions += 1,
                ActionSafety::Destructive => summary.destructive_actions += 1,
            }
        }

        summary
    }
}

fn stable_plan_id(
    schema_version: PlanSchemaVersion,
    project: &ProjectIdentity,
    operation: PlannedOperation,
    actions: &IndexMap<ActionId, PlanActionNode>,
) -> PlanId {
    let mut builder = StableIdBuilder::new();
    builder.part(&schema_version.major.to_string());
    builder.part(&schema_version.minor.to_string());
    builder.part(project.working_set.as_str());
    builder.part(operation.as_str());
    for id in actions.keys() {
        builder.part(id.as_str());
    }
    PlanId::from_parts(&[&format!("{:016x}", builder.finish())])
}
