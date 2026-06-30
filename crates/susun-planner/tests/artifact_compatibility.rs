//! Persisted artifact compatibility coverage.

use std::error::Error;

use susun_planner::ExecutionPlan;

#[test]
fn current_plan_schema_accepts_minimal_v1_fixture() -> Result<(), Box<dyn Error>> {
    let json = r#"
    {
      "schema_version": { "major": 1, "minor": 0 },
      "plan_id": "plan-fixture",
      "project": {
        "name": "compat",
        "working_set": "compat-fixture"
      },
      "operation": "up",
      "created_at": null,
      "actions": {
        "act-pull": {
          "id": "act-pull",
          "action": {
            "type": "pull_image",
            "payload": {
              "image": "busybox:latest"
            }
          },
          "dependencies": [],
          "reason": {
            "code": "image_unavailable_locally",
            "message": "image is unavailable locally"
          },
          "safety": "safe"
        }
      },
      "summary": {
        "total_actions": 1,
        "safe_actions": 1,
        "caution_actions": 0,
        "destructive_actions": 0
      }
    }
    "#;

    let plan: ExecutionPlan = serde_json::from_str(json)?;

    assert_eq!(plan.schema_version.major, 1);
    assert_eq!(plan.schema_version.minor, 0);
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(plan.summary.total_actions, 1);
    Ok(())
}
