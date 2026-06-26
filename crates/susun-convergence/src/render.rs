//! Human-readable convergence rendering.

use crate::ConvergenceOutcome;

/// Renders convergence decisions for human review.
pub fn render_convergence_human(outcome: &ConvergenceOutcome) -> String {
    let mut output = String::new();
    output.push_str("Convergence decisions\n");
    for (instance, decision) in &outcome.decisions {
        output.push_str(&format!(
            "- {}/{}[{}]: {:?} -> {:?}",
            instance.project.as_str(),
            instance.service.as_str(),
            instance.replica.as_u32(),
            decision.difference,
            decision.kind
        ));
        if decision.destructive {
            output.push_str(" destructive");
        }
        if decision.downtime_expected {
            output.push_str(" downtime_expected");
        }
        output.push('\n');
    }
    output
}
