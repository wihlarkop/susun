//! Pure execution planning for Susun projects.
//!
//! The planner converts Phase 1 analysis outputs plus explicit neutral engine
//! inputs into deterministic, explainable execution plans. It performs no
//! daemon calls and does not mutate the host.

pub mod dag;
pub mod id;
pub mod naming;
pub mod plan;

pub use dag::{topological_action_order, validate_action_dag};
pub use id::{ActionId, PlanId, StableIdBuilder};
pub use naming::{ComposeCompatibleNamingPolicy, NamingError, NamingPolicy, SusunNamingPolicy};
pub use plan::{
    ActionExplanation, ActionReason, ActionSafety, CreateContainerAction, CreateNetworkAction,
    CreateVolumeAction, ExecutionPlan, NoOpAction, PlanAction, PlanActionNode, PlanSchemaVersion,
    PlanSummary, PlannedOperation, PullImageAction, RemoveContainerAction, RemoveNetworkAction,
    RemoveVolumeAction, StartContainerAction, StopContainerAction, WaitForDependencyAction,
};
