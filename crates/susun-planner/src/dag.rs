//! Action DAG validation and traversal.

use indexmap::{IndexMap, IndexSet};
use thiserror::Error;

use crate::{ActionId, PlanActionNode};

/// Planner invariant error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanError {
    /// Internal planner invariant was violated.
    #[error("planner invariant violated: {detail}")]
    InvariantViolation {
        /// Invariant failure detail.
        detail: String,
    },
    /// A generated action ID collided with an existing action.
    #[error("action id collision: {id}")]
    ActionIdCollision {
        /// Colliding action ID.
        id: ActionId,
    },
    /// An action references a dependency that is not present in the plan.
    #[error("action {action} references missing dependency {dependency}")]
    InvalidDependencyReference {
        /// Action with invalid dependency.
        action: ActionId,
        /// Missing dependency ID.
        dependency: ActionId,
    },
}

/// Validates that all action dependencies form a complete acyclic graph.
pub fn validate_action_dag(actions: &IndexMap<ActionId, PlanActionNode>) -> Result<(), PlanError> {
    topological_action_order(actions).map(|_| ())
}

/// Returns a stable topological action order.
pub fn topological_action_order(
    actions: &IndexMap<ActionId, PlanActionNode>,
) -> Result<Vec<ActionId>, PlanError> {
    let mut dependents: IndexMap<ActionId, IndexSet<ActionId>> = IndexMap::new();
    let mut indegree: IndexMap<ActionId, usize> = IndexMap::new();

    for (id, node) in actions {
        if id != &node.id {
            return Err(PlanError::InvariantViolation {
                detail: format!("action key {id} does not match node id {}", node.id),
            });
        }
        indegree.insert(id.clone(), node.dependencies.len());
        dependents.entry(id.clone()).or_default();

        for dependency in &node.dependencies {
            if dependency == id {
                return Err(PlanError::InvariantViolation {
                    detail: format!("action {id} depends on itself"),
                });
            }
            if !actions.contains_key(dependency) {
                return Err(PlanError::InvalidDependencyReference {
                    action: id.clone(),
                    dependency: dependency.clone(),
                });
            }
            dependents
                .entry(dependency.clone())
                .or_default()
                .insert(id.clone());
        }
    }

    let mut ready = actions
        .keys()
        .filter(|id| indegree.get(*id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect::<Vec<_>>();
    let mut ordered = Vec::with_capacity(actions.len());

    while let Some(id) = ready.first().cloned() {
        ready.remove(0);
        ordered.push(id.clone());

        let Some(children) = dependents.get(&id) else {
            continue;
        };

        for child in children {
            let Some(count) = indegree.get_mut(child) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                ready.push(child.clone());
            }
        }
    }

    if ordered.len() != actions.len() {
        return Err(PlanError::InvariantViolation {
            detail: "action graph contains a cycle".to_owned(),
        });
    }

    Ok(ordered)
}
