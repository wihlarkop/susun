#![allow(missing_docs)]

use std::{error::Error, path::PathBuf};

use susun::{
    Analyzer, BuildPolicy, EngineCapabilities, EngineSnapshot, Error as SusunError, Project,
    ProjectIdentity, ProjectInstanceId, ProjectName, ProjectSummarySchemaVersion, SusunWorkspace,
    UpPlanOptions, parse_engine_connection_profile_set_json, parse_execution_plan_json,
    parse_execution_report_json, parse_project_summary_json,
    render_engine_connection_profile_set_json, render_project_summary_json,
};
use susun::{render_execution_plan_json, render_execution_report_json};
use susun_runtime::ExecutionReport;

type TestResult = Result<(), Box<dyn Error>>;

fn valid_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/valid-minimal/compose.yaml")
}

fn malformed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/malformed/compose.yaml")
}

fn resources_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/compatibility/resources-configs-secrets/compose.yaml")
}

#[test]
fn valid_file_produces_canonical_project() -> TestResult {
    let result = Analyzer::new(valid_path()).analyze()?;
    let project = result.project.ok_or("expected a project")?;
    assert_eq!(project.name.to_string(), "valid-minimal");
    let key = project
        .services
        .keys()
        .next()
        .ok_or("expected at least one service")?;
    assert_eq!(key.as_str(), "web");
    let service = project
        .services
        .values()
        .next()
        .ok_or("expected service value")?;
    let image = service.image.as_ref().ok_or("expected image")?;
    assert_eq!(image.as_str(), "nginx:latest");
    Ok(())
}

#[test]
fn malformed_file_returns_ok_with_error_report() -> TestResult {
    let result = Analyzer::new(malformed_path()).analyze()?;
    assert!(result.report.has_errors(), "expected error diagnostics");
    assert!(result.project.is_none());
    Ok(())
}

#[test]
fn missing_file_returns_load_error() {
    let err = Analyzer::new("/nonexistent/compose.yaml").analyze().err();
    assert!(matches!(err, Some(SusunError::Load(_))));
}

#[test]
fn valid_file_report_is_clean() -> TestResult {
    let result = Analyzer::new(valid_path()).analyze()?;
    assert!(!result.report.has_errors());
    assert!(result.report.is_empty(), "expected no diagnostics at all");
    Ok(())
}

#[test]
fn workspace_summary_is_structured_for_sdk_consumers() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let summary = project.summary();

    assert_eq!(summary.schema_version, ProjectSummarySchemaVersion::CURRENT);
    assert_eq!(summary.project_name.as_deref(), Some("valid-minimal"));
    assert_eq!(summary.service_count, 1);
    assert_eq!(summary.active_service_count, 1);
    assert!(!summary.has_errors);
    assert_eq!(summary.diagnostic_count, 0);
    assert_eq!(summary.services[0].name, "web");
    assert_eq!(summary.services[0].image.as_deref(), Some("nginx:latest"));
    assert!(summary.services[0].active);
    assert!(summary.project_instance.is_some());

    let json = serde_json::to_value(&summary)?;
    assert_eq!(json["schema_version"]["major"], 1);
    assert_eq!(json["schema_version"]["minor"], 0);
    assert_eq!(json["services"][0]["name"], "web");

    let roundtrip: susun::ProjectSummary = serde_json::from_value(json)?;
    assert_eq!(roundtrip, summary);

    let rendered = render_project_summary_json(&summary)?;
    let parsed = parse_project_summary_json(&rendered)?;
    assert_eq!(parsed, summary);
    Ok(())
}

#[test]
fn workspace_summary_exposes_resource_references_without_secret_values() -> TestResult {
    let project = SusunWorkspace::from_file(resources_path()).analyze()?;
    let summary = project.summary();

    assert_eq!(summary.project_name.as_deref(), Some("compat-resources"));
    assert_eq!(summary.config_count, 1);
    assert_eq!(summary.secret_count, 1);
    assert_eq!(summary.configs[0].name, "app_config");
    assert_eq!(summary.secrets[0].name, "app_secret");

    let service = &summary.services[0];
    assert_eq!(service.name, "worker");
    assert_eq!(service.config_count, 1);
    assert_eq!(service.secret_count, 1);
    assert_eq!(service.configs, vec!["app_config"]);
    assert_eq!(service.secrets, vec!["app_secret"]);

    let json = serde_json::to_string(&summary)?;
    assert!(!json.contains("super-secret"));
    assert!(!json.contains("secret file contents"));
    Ok(())
}

#[test]
fn workspace_dry_run_plan_uses_facade_defaults() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let outcome = project.dry_run_up(false)?;

    assert!(!outcome.diagnostics.has_errors());
    assert!(outcome.plan.is_some(), "expected a daemon-free up plan");
    Ok(())
}

#[test]
fn workspace_dry_run_down_plan_uses_facade_defaults() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let outcome = project.dry_run_down()?;

    assert!(!outcome.diagnostics.has_errors());
    assert!(outcome.plan.is_some(), "expected a daemon-free down plan");
    assert!(project.dry_run_down_plan()?.is_some());
    Ok(())
}

#[test]
fn facade_execution_plan_json_helpers_roundtrip() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let plan = project
        .dry_run_up_plan(false)?
        .ok_or("expected daemon-free up plan")?;

    let json = render_execution_plan_json(&plan)?;
    let parsed = parse_execution_plan_json(&json)?;

    assert_eq!(parsed.plan_id, plan.plan_id);
    assert_eq!(parsed.schema_version, plan.schema_version);
    assert_eq!(parsed.summary, plan.summary);
    assert_eq!(parsed.actions.len(), plan.actions.len());
    Ok(())
}

#[test]
fn facade_execution_report_json_helpers_roundtrip() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let plan = project
        .dry_run_up_plan(false)?
        .ok_or("expected daemon-free up plan")?;
    let report = ExecutionReport::pending(&plan);

    let json = render_execution_report_json(&report)?;
    let parsed = parse_execution_report_json(&json)?;

    assert_eq!(parsed.plan_id, report.plan_id);
    assert_eq!(parsed.summary.total_actions, report.summary.total_actions);
    assert_eq!(parsed.actions.len(), report.actions.len());
    Ok(())
}

#[test]
fn facade_reexports_common_sdk_types() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let canonical: &Project = project.project().ok_or("expected project")?;
    let name = ProjectName::new("sdk");
    let identity = ProjectIdentity::new(name.clone(), ProjectInstanceId::derive(&name, "."));
    let capabilities = EngineCapabilities::permissive_local();
    let snapshot = EngineSnapshot::empty(std::time::SystemTime::UNIX_EPOCH);
    let options = UpPlanOptions {
        build_policy: BuildPolicy::NeverBuild,
        ..UpPlanOptions::default()
    };

    assert_eq!(canonical.name.as_str(), "valid-minimal");
    let outcome = project.plan_up(capabilities, snapshot, options)?;
    assert!(outcome.plan.is_some());
    assert_eq!(identity.name.as_str(), "sdk");
    Ok(())
}

#[test]
fn facade_reexports_runtime_readiness_types() -> TestResult {
    let profile = susun::EngineConnectionProfile::new(
        susun::EngineConnectionProfileId::new("local")?,
        susun::EngineConnectionDisplayName::new("Local Docker")?,
        susun::EngineEndpoint::Local,
    );
    let status = susun::RuntimeDoctorStatus::Available;

    assert_eq!(profile.redacted_endpoint(), "local");
    assert_eq!(profile.display_name.as_str(), "Local Docker");
    assert!(matches!(status, susun::RuntimeDoctorStatus::Available));
    Ok(())
}

#[test]
fn facade_reexports_runtime_profile_set() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::local_default(),
    ])?;

    assert_eq!(
        set.default_profile()
            .ok_or("expected default profile")?
            .id
            .as_str(),
        "local"
    );
    Ok(())
}

#[test]
fn facade_runtime_profile_json_helpers_roundtrip() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::local_default(),
    ])?;

    let json = render_engine_connection_profile_set_json(&set)?;
    let parsed = parse_engine_connection_profile_set_json(&json)?;

    assert_eq!(
        parsed
            .default_profile()
            .ok_or("expected default profile")?
            .id
            .as_str(),
        "local"
    );
    Ok(())
}

#[test]
fn facade_runtime_profile_json_helpers_reject_duplicate_ids() {
    let json = r#"{
        "profiles": [
            {
                "id": "local",
                "display_name": "Local A",
                "endpoint": "Local",
                "default": false
            },
            {
                "id": "local",
                "display_name": "Local B",
                "endpoint": "Local",
                "default": false
            }
        ]
    }"#;

    let result = parse_engine_connection_profile_set_json(json);
    assert!(result.is_err());
}
