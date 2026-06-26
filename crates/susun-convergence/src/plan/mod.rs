//! Pure convergence plan fragments.

use indexmap::IndexMap;
use susun_planner::{ActionId, PlanActionNode};

pub mod noop;
pub mod recreate;
pub mod scale;

pub use noop::plan_noop_or_start;
pub use recreate::{ReplacementInput, plan_replacement};
pub use scale::plan_scale;

/// A deterministic set of convergence action nodes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConvergencePlanFragment {
    /// Action nodes keyed by stable ID.
    pub actions: IndexMap<ActionId, PlanActionNode>,
}

impl ConvergencePlanFragment {
    /// Creates an empty fragment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts one action node.
    pub fn insert(&mut self, node: PlanActionNode) {
        self.actions.insert(node.id.clone(), node);
    }
}
