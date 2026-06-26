//! Dependent impact propagation for convergence.

use indexmap::IndexSet;
use susun_engine::{ReplicaIndex, ServiceInstanceId};
use susun_model::{DependencyCondition, ServiceName};

use crate::{DependencyRecreatePolicy, DesiredDeployment};

/// Dependency impact classification for a set of changed instances.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyImpact {
    /// Instances directly changed by fingerprint/diff classification.
    pub changed: IndexSet<ServiceInstanceId>,
    /// Dependents that should restart after a changed dependency.
    pub restart_dependents: IndexSet<ServiceInstanceId>,
    /// Dependents that must be recreated by policy.
    pub recreate_dependents: IndexSet<ServiceInstanceId>,
    /// Dependent/dependency pairs that require readiness ordering.
    pub readiness_waits: Vec<DependencyWait>,
}

/// Readiness edge introduced by dependency impact propagation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyWait {
    /// Dependent service instance.
    pub dependent: ServiceInstanceId,
    /// Changed dependency service instance.
    pub dependency: ServiceInstanceId,
    /// Required condition key.
    pub condition: DependencyCondition,
}

/// Propagates direct service changes to affected dependents according to policy.
pub fn propagate_dependency_impact(
    desired: &DesiredDeployment,
    changed: &IndexSet<ServiceInstanceId>,
    policy: DependencyRecreatePolicy,
) -> DependencyImpact {
    let mut impact = DependencyImpact {
        changed: changed.clone(),
        ..DependencyImpact::default()
    };

    for dependency in changed {
        let Some(dependents) = desired.graph.edges.get(&dependency.service) else {
            continue;
        };

        for dependent_name in dependents {
            let Some(dependent_service) = desired.project.services.get(dependent_name) else {
                continue;
            };
            let Some(edge) = dependent_service.depends_on.get(&dependency.service) else {
                continue;
            };

            let dependent = first_replica(&desired.identity.working_set, dependent_name);
            if edge.restart && policy != DependencyRecreatePolicy::Never {
                impact.restart_dependents.insert(dependent.clone());
            }
            if edge.condition == DependencyCondition::ServiceHealthy {
                impact.readiness_waits.push(DependencyWait {
                    dependent: dependent.clone(),
                    dependency: dependency.clone(),
                    condition: edge.condition,
                });
            }
            if policy == DependencyRecreatePolicy::RecreateWhenDependencyContractRequires
                && edge.restart
                && edge.condition == DependencyCondition::ServiceHealthy
            {
                impact.recreate_dependents.insert(dependent);
            }
        }
    }

    impact
}

fn first_replica(
    project: &susun_engine::ProjectInstanceId,
    service: &ServiceName,
) -> ServiceInstanceId {
    ServiceInstanceId::new(project.clone(), service.clone(), ReplicaIndex::FIRST)
}
