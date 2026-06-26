//! Desired replica expansion and scale delta classification.

use indexmap::IndexSet;
use susun_engine::{ReplicaIndex, ServiceInstanceId};

use crate::{DesiredDeployment, ObservedDeployment};

/// Deterministic scale delta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScaleDelta {
    /// Missing replicas to create.
    pub scale_up: Vec<ServiceInstanceId>,
    /// Extra replicas to remove, highest replica index first.
    pub scale_down: Vec<ServiceInstanceId>,
}

/// Expands desired service replica counts into stable instance identities.
pub fn expand_desired_replicas(desired: &DesiredDeployment) -> Vec<ServiceInstanceId> {
    let mut instances = Vec::new();
    for (service, count) in &desired.replicas {
        for replica in 0..count.get() {
            instances.push(ServiceInstanceId::new(
                desired.identity.working_set.clone(),
                service.clone(),
                ReplicaIndex::new(replica),
            ));
        }
    }
    instances
}

/// Classifies observed replicas into scale-up and scale-down work.
pub fn classify_scale_delta(
    desired: &DesiredDeployment,
    observed: &ObservedDeployment,
) -> ScaleDelta {
    let desired_instances = expand_desired_replicas(desired)
        .into_iter()
        .collect::<IndexSet<_>>();
    let observed_instances = observed
        .ownership
        .containers
        .keys()
        .cloned()
        .collect::<IndexSet<_>>();
    let mut scale_up = desired_instances
        .iter()
        .filter(|instance| !observed_instances.contains(*instance))
        .cloned()
        .collect::<Vec<_>>();
    let mut scale_down = observed_instances
        .iter()
        .filter(|instance| {
            instance.project == desired.identity.working_set
                && !desired_instances.contains(*instance)
        })
        .cloned()
        .collect::<Vec<_>>();

    scale_up.sort();
    scale_down.sort_by(|left, right| right.cmp(left));
    ScaleDelta {
        scale_up,
        scale_down,
    }
}
