//! Post-execution convergence verification contracts.

use susun_engine::{EngineSnapshot, ServiceInstanceId};
use susun_planner::ExecutionPlan;

/// Verification finding after execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationFinding {
    /// Affected service instance.
    pub instance: ServiceInstanceId,
    /// Redacted finding message.
    pub message: String,
}

/// Verification report that never replaces the execution report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerificationReport {
    /// Verification findings.
    pub findings: Vec<VerificationFinding>,
}

impl VerificationReport {
    /// Returns whether verification found no issues.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Verifies that planned service actions have observable resources.
pub fn verify_execution_snapshot(
    plan: &ExecutionPlan,
    snapshot: &EngineSnapshot,
) -> VerificationReport {
    let mut report = VerificationReport::default();
    for node in plan.actions.values() {
        let Some(instance) = service_instance(&node.action) else {
            continue;
        };
        if !snapshot
            .containers
            .values()
            .any(|container| container.service_identity.as_ref() == Some(instance))
        {
            report.findings.push(VerificationFinding {
                instance: instance.clone(),
                message: "expected service instance was not observed after execution".to_string(),
            });
        }
    }
    report
}

fn service_instance(action: &susun_planner::PlanAction) -> Option<&ServiceInstanceId> {
    match action {
        susun_planner::PlanAction::CreateContainer(action) => Some(&action.identity),
        susun_planner::PlanAction::VerifyBuildInputs(action) => Some(&action.identity),
        susun_planner::PlanAction::BuildImage(action) => Some(&action.identity),
        susun_planner::PlanAction::StartContainer(action) => Some(&action.identity),
        susun_planner::PlanAction::VerifyReplacement(action) => Some(&action.identity),
        susun_planner::PlanAction::ScaleUpReplica(action) => Some(&action.identity),
        _ => None,
    }
}
