//! Human-readable plan rendering.

use crate::{ActionSafety, ExecutionPlan, PlanAction, topological_action_order};

/// Renders a plan for human review.
pub fn render_plan_human(plan: &ExecutionPlan) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Plan {} ({:?})\n",
        plan.plan_id.as_str(),
        plan.operation
    ));
    output.push_str(&format!(
        "Project: {} [{}]\n",
        plan.project.name.as_str(),
        plan.project.working_set.as_str()
    ));
    output.push_str(&format!(
        "Actions: {} safe, {} caution, {} destructive, {} total\n",
        plan.summary.safe_actions,
        plan.summary.caution_actions,
        plan.summary.destructive_actions,
        plan.summary.total_actions
    ));

    let ordered_ids = topological_action_order(&plan.actions)
        .unwrap_or_else(|_| plan.actions.keys().cloned().collect());

    for id in ordered_ids {
        let Some(node) = plan.actions.get(&id) else {
            continue;
        };
        output.push_str(&format!(
            "- {} [{}] {}: {}\n",
            node.id.as_str(),
            safety_label(node.safety),
            action_label(&node.action),
            node.reason.message
        ));
        if !node.dependencies.is_empty() {
            let dependencies = node
                .dependencies
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!("  depends_on: {dependencies}\n"));
        }
    }

    output
}

fn safety_label(safety: ActionSafety) -> &'static str {
    match safety {
        ActionSafety::Safe => "safe",
        ActionSafety::Caution => "caution",
        ActionSafety::Destructive => "destructive",
    }
}

fn action_label(action: &PlanAction) -> String {
    match action {
        PlanAction::VerifyBuildInputs(action) => {
            format!("verify build inputs {}", action.identity.service.as_str())
        }
        PlanAction::BuildImage(action) => {
            format!("build image {}", action.identity.service.as_str())
        }
        PlanAction::PullImage(action) => format!("pull image {}", action.image.as_str()),
        PlanAction::CreateNetwork(action) => {
            format!("create network {}", action.name.as_str())
        }
        PlanAction::CreateVolume(action) => format!("create volume {}", action.name.as_str()),
        PlanAction::CreateContainer(action) => {
            format!("create container {}", action.name.as_str())
        }
        PlanAction::StartContainer(action) => {
            format!("start service {}", action.identity.service.as_str())
        }
        PlanAction::WaitForDependency(action) => format!(
            "wait for {} before {}",
            action.dependency.service.as_str(),
            action.dependent.service.as_str()
        ),
        PlanAction::StopContainer(action) => {
            format!("stop service {}", action.identity.service.as_str())
        }
        PlanAction::RemoveContainer(action) => {
            format!("remove service {}", action.identity.service.as_str())
        }
        PlanAction::RemoveNetwork(action) => {
            format!("remove network {}", action.identity.network.as_str())
        }
        PlanAction::RemoveVolume(action) => {
            format!("remove volume {}", action.identity.volume.as_str())
        }
        PlanAction::RenameContainer(action) => format!(
            "rename service {} from {} to {}",
            action.identity.service.as_str(),
            action.from.as_str(),
            action.to.as_str()
        ),
        PlanAction::RecreateContainer(action) => format!(
            "recreate service {} using {}",
            action.identity.service.as_str(),
            action.strategy
        ),
        PlanAction::PreserveVolume(action) => {
            format!("preserve volume {}", action.identity.volume.as_str())
        }
        PlanAction::VerifyReplacement(action) => {
            format!(
                "verify replacement for {}",
                action.identity.service.as_str()
            )
        }
        PlanAction::RemoveOrphan(action) => {
            format!("remove orphan {} {}", action.kind, action.resource)
        }
        PlanAction::ScaleUpReplica(action) => {
            format!("scale up {}", action.identity.service.as_str())
        }
        PlanAction::ScaleDownReplica(action) => {
            format!("scale down {}", action.identity.service.as_str())
        }
        PlanAction::NoOp(action) => format!("no-op {}", action.resource),
    }
}
