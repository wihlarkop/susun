//! JSON plan rendering.

use crate::ExecutionPlan;

/// Renders a plan as pretty JSON.
pub fn render_plan_json(plan: &ExecutionPlan) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(plan)
}
