//! Desired-versus-observed difference classification.

use indexmap::IndexMap;
use susun_engine::{
    ConfigurationFingerprint, ContainerState, ObservedContainer, ObservedImageRef, ReplicaIndex,
    ServiceInstanceId,
};

use crate::{
    ChangedField, DesiredDeployment, InstanceDifference, ObservedDeployment, RuntimeDrift,
};

/// Desired configuration fingerprints by service instance.
pub type DesiredInstanceFingerprints = IndexMap<ServiceInstanceId, ConfigurationFingerprint>;

/// Classifies all desired service instances against observed state.
pub fn classify_deployment_differences(
    desired: &DesiredDeployment,
    observed: &ObservedDeployment,
    desired_fingerprints: &DesiredInstanceFingerprints,
) -> IndexMap<ServiceInstanceId, InstanceDifference> {
    let mut differences = IndexMap::new();

    for (service, replicas) in &desired.replicas {
        for replica in 0..replicas.get() {
            let instance = ServiceInstanceId::new(
                desired.identity.working_set.clone(),
                service.clone(),
                ReplicaIndex::new(replica),
            );
            let difference = classify_instance(&instance, observed, desired_fingerprints);
            differences.insert(instance, difference);
        }
    }

    differences
}

/// Classifies one desired service instance against observed state.
pub fn classify_instance(
    instance: &ServiceInstanceId,
    observed: &ObservedDeployment,
    desired_fingerprints: &DesiredInstanceFingerprints,
) -> InstanceDifference {
    if observed
        .ownership
        .duplicate_claims
        .iter()
        .any(|conflict| &conflict.instance == instance)
    {
        return InstanceDifference::OwnershipAmbiguous;
    }

    let Some(container) = observed.ownership.containers.get(instance) else {
        return InstanceDifference::Missing;
    };

    classify_container(container, desired_fingerprints.get(instance))
}

/// Classifies one observed container against its desired fingerprint.
pub fn classify_container(
    container: &ObservedContainer,
    desired_fingerprint: Option<&ConfigurationFingerprint>,
) -> InstanceDifference {
    if matches!(container.state, ContainerState::Unknown)
        || matches!(container.image, ObservedImageRef::Unknown)
    {
        return InstanceDifference::UnsupportedObservedState;
    }

    let Some(desired_fingerprint) = desired_fingerprint else {
        return InstanceDifference::ConfigurationChanged {
            fields: vec![ChangedField::new(
                "configuration_fingerprint.missing_desired",
            )],
        };
    };

    let Some(observed_fingerprint) = &container.configuration_fingerprint else {
        return InstanceDifference::ConfigurationChanged {
            fields: vec![ChangedField::new(
                "configuration_fingerprint.missing_observed",
            )],
        };
    };

    if observed_fingerprint != desired_fingerprint {
        return InstanceDifference::ConfigurationChanged {
            fields: vec![ChangedField::new("configuration_fingerprint")],
        };
    }

    match container.state {
        ContainerState::Running => InstanceDifference::Unchanged,
        ContainerState::Created | ContainerState::Exited => {
            InstanceDifference::StoppedButCompatible
        }
        ContainerState::Paused => InstanceDifference::RuntimeStateDrift {
            drift: RuntimeDrift::Paused,
        },
        ContainerState::Restarting => InstanceDifference::RuntimeStateDrift {
            drift: RuntimeDrift::Restarting,
        },
        ContainerState::Unknown => InstanceDifference::UnsupportedObservedState,
    }
}
