//! Pure execution planning for Susun projects.
//!
//! The planner converts Phase 1 analysis outputs plus explicit neutral engine
//! inputs into deterministic, explainable execution plans. It performs no
//! daemon calls and does not mutate the host.

pub mod capability_check;
pub mod dag;
pub mod down;
pub mod error;
pub mod id;
pub mod input;
pub mod naming;
pub mod options;
pub mod ownership;
pub mod plan;
pub mod render;
pub mod up;

pub use capability_check::check_up_capabilities;
pub use dag::{topological_action_order, validate_action_dag};
pub use down::plan_down;
pub use error::PlanError;
pub use id::{ActionId, PlanId, StableIdBuilder};
pub use input::{PlanOutcome, PlanningInput};
pub use naming::{ComposeCompatibleNamingPolicy, NamingError, NamingPolicy, SusunNamingPolicy};
pub use options::{
    BuildPolicy, DependencyWaitPolicy, DownPlanOptions, ExistingResourcePolicy,
    ImageAcquisitionPolicy, UpPlanOptions,
};
pub use ownership::{OwnedResourceIndex, index_owned_resources};
pub use plan::{
    ActionExplanation, ActionReason, ActionSafety, BuildImageAction, CreateContainerAction,
    CreateNetworkAction, CreateVolumeAction, ExecutionPlan, NoOpAction, PlanAction, PlanActionNode,
    PlanSchemaVersion, PlanSummary, PlannedOperation, PreserveVolumeAction, PullImageAction,
    RecreateContainerAction, RemoveContainerAction, RemoveNetworkAction, RemoveOrphanAction,
    RemoveVolumeAction, RenameContainerAction, ScaleDownReplicaAction, ScaleUpReplicaAction,
    StartContainerAction, StopContainerAction, VerifyBuildInputsAction, VerifyReplacementAction,
    WaitForDependencyAction,
};
pub use render::render_plan_human;
#[cfg(feature = "serde")]
pub use render::render_plan_json;
pub use up::{UpResourceActions, plan_up};
