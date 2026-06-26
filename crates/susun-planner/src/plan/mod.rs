//! Versioned execution plan schema.

pub mod action;
pub mod reason;
pub mod schema;

pub use action::{
    BuildImageAction, CreateContainerAction, CreateNetworkAction, CreateVolumeAction,
    ExecutionPlan, NoOpAction, PlanAction, PlanActionNode, PlanSummary, PlannedOperation,
    PreserveVolumeAction, PullImageAction, RecreateContainerAction, RemoveContainerAction,
    RemoveNetworkAction, RemoveOrphanAction, RemoveVolumeAction, RenameContainerAction,
    ScaleDownReplicaAction, ScaleUpReplicaAction, StartContainerAction, StopContainerAction,
    VerifyBuildInputsAction, VerifyReplacementAction, WaitForDependencyAction,
};
pub use reason::{ActionExplanation, ActionReason, ActionSafety};
pub use schema::PlanSchemaVersion;
