//! Versioned execution plan schema.

pub mod action;
pub mod reason;
pub mod schema;

pub use action::{
    CreateContainerAction, CreateNetworkAction, CreateVolumeAction, ExecutionPlan, NoOpAction,
    PlanAction, PlanActionNode, PlanSummary, PlannedOperation, PreserveVolumeAction,
    PullImageAction, RecreateContainerAction, RemoveContainerAction, RemoveNetworkAction,
    RemoveVolumeAction, RenameContainerAction, StartContainerAction, StopContainerAction,
    VerifyReplacementAction, WaitForDependencyAction,
};
pub use reason::{ActionExplanation, ActionReason, ActionSafety};
pub use schema::PlanSchemaVersion;
