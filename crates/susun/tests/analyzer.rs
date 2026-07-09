#![allow(missing_docs)]

use std::{error::Error, path::PathBuf};

use susun::{
    AnalysisErrorKind, AnalysisErrorSummary, AnalysisErrorSummarySchemaVersion, Analyzer,
    BuildPolicy, DependencyGraph, DiagnosticReportSummarySchemaVersion, EngineCapabilities,
    EngineSnapshot, Error as SusunError, Project, ProjectIdentity, ProjectInstanceId, ProjectName,
    ProjectSelection, ProjectSummarySchemaVersion, SourceMap, SusunWorkspace, UpPlanOptions,
    parse_analysis_error_summary_json, parse_diagnostic_report_summary_json,
    parse_engine_connection_profile_set_json, parse_engine_connection_profile_set_summary_json,
    parse_execution_plan_json, parse_execution_report_json, parse_plan_outcome_summary_json,
    parse_project_summary_json, render_analysis_error_summary_json,
    render_diagnostic_report_summary_json, render_engine_connection_profile_set_json,
    render_engine_connection_profile_set_summary_json, render_project_summary_json,
};
use susun::{
    EngineConnectionProfileSetSummary, EngineConnectionProfileSetSummarySchemaVersion,
    PlanOutcomeSummary, PlanOutcomeSummarySchemaVersion, render_execution_plan_json,
    render_execution_report_json, render_plan_outcome_summary_json,
};
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
fn analysis_error_summary_redacts_load_paths() -> TestResult {
    let error = Analyzer::new("/very/private/missing-compose.yaml")
        .analyze()
        .err()
        .ok_or("expected load error")?;
    let summary = AnalysisErrorSummary::from(&error);

    assert_eq!(
        summary.schema_version,
        AnalysisErrorSummarySchemaVersion::CURRENT
    );
    assert_eq!(summary.kind, AnalysisErrorKind::LoadNotFound);
    assert_eq!(summary.message, "compose file was not found");
    assert!(!summary.message.contains("very/private"));
    assert!(!summary.message.contains("missing-compose"));

    let json = render_analysis_error_summary_json(&summary)?;
    assert!(!json.contains("very/private"));
    assert!(!json.contains("missing-compose"));
    let parsed = parse_analysis_error_summary_json(&json)?;
    assert_eq!(parsed, summary);
    Ok(())
}

#[test]
fn analysis_error_summary_json_helper_rejects_unsupported_schema_version() -> TestResult {
    let error = Analyzer::new("/very/private/missing-compose.yaml")
        .analyze()
        .err()
        .ok_or("expected load error")?;
    let summary = AnalysisErrorSummary::from(&error);
    let json = render_analysis_error_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["schema_version"]["minor"] = serde_json::json!(1);

    let result = parse_analysis_error_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn analysis_error_summary_json_helper_rejects_message_drift() -> TestResult {
    let error = Analyzer::new("/very/private/missing-compose.yaml")
        .analyze()
        .err()
        .ok_or("expected load error")?;
    let summary = AnalysisErrorSummary::from(&error);
    let json = render_analysis_error_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["message"] = serde_json::json!("raw /very/private/missing-compose.yaml");

    let result = parse_analysis_error_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn valid_file_report_is_clean() -> TestResult {
    let result = Analyzer::new(valid_path()).analyze()?;
    assert!(!result.report.has_errors());
    assert!(result.report.is_empty(), "expected no diagnostics at all");
    Ok(())
}

#[test]
fn workspace_exposes_default_configuration_for_sdk_consumers() {
    let workspace = SusunWorkspace::new();

    assert!(workspace.files().is_empty());
    assert_eq!(workspace.primary_file(), PathBuf::from("compose.yaml"));
    assert_eq!(workspace.env_file(), None);
    assert_eq!(workspace.env_vars(), None);
    assert_eq!(workspace.project_name(), None);
    assert!(workspace.profiles().is_empty());
}

#[test]
fn workspace_exposes_configured_options_for_sdk_consumers() {
    let primary = PathBuf::from("compose.yaml");
    let override_file = PathBuf::from("compose.override.yaml");
    let env_file = PathBuf::from(".env.local");
    let workspace = SusunWorkspace::from_file(primary.clone())
        .with_file(override_file.clone())
        .with_env_file(env_file.clone())
        .with_env_var("COMPOSE_PROJECT_NAME", "sdk-app")
        .with_project_name("explicit-name")
        .with_profiles(["debug", "worker"]);

    assert_eq!(workspace.files(), &[primary, override_file]);
    assert_eq!(workspace.env_file(), Some(env_file.as_path()));
    assert_eq!(
        workspace
            .env_vars()
            .and_then(|vars| vars.get("COMPOSE_PROJECT_NAME"))
            .map(String::as_str),
        Some("sdk-app")
    );
    assert_eq!(workspace.project_name(), Some("explicit-name"));
    assert_eq!(
        workspace.profiles(),
        &["debug".to_owned(), "worker".to_owned()]
    );
}

#[test]
fn workspace_summary_is_structured_for_sdk_consumers() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let summary = project.summary();
    let converted = susun::ProjectSummary::from(&project);

    assert_eq!(converted, summary);
    assert!(!project.has_errors());
    assert_eq!(project.diagnostic_count(), 0);
    assert!(project.diagnostics().is_empty());
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
fn project_summary_json_helper_rejects_unsupported_schema_version() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let summary = project.summary();
    let json = render_project_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["schema_version"]["major"] = serde_json::json!(2);

    let result = parse_project_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn project_summary_json_helper_rejects_inconsistent_counts() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let summary = project.summary();
    let json = render_project_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["services"][0]["port_count"] = serde_json::json!(999);

    let result = parse_project_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn project_summary_json_helper_rejects_invalid_identity_fields() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let summary = project.summary();
    let json = render_project_summary_json(&summary)?;

    let mut missing_project: serde_json::Value = serde_json::from_str(&json)?;
    missing_project["project_name"] = serde_json::json!("");
    assert!(parse_project_summary_json(&serde_json::to_string(&missing_project)?).is_err());

    let mut missing_instance: serde_json::Value = serde_json::from_str(&json)?;
    missing_instance["project_instance"] = serde_json::json!(" ");
    assert!(parse_project_summary_json(&serde_json::to_string(&missing_instance)?).is_err());

    let mut empty_service: serde_json::Value = serde_json::from_str(&json)?;
    empty_service["services"][0]["name"] = serde_json::json!("");
    assert!(parse_project_summary_json(&serde_json::to_string(&empty_service)?).is_err());
    Ok(())
}

#[test]
fn project_summary_json_helper_rejects_active_count_drift() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let summary = project.summary();
    let json = render_project_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["services"][0]["active"] = serde_json::json!(false);

    let result = parse_project_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn workspace_exposes_analysis_components_without_analysis_plumbing() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let canonical: &Project = project.project().ok_or("expected project")?;
    let selection: &ProjectSelection = project.selection().ok_or("expected selection")?;
    let graph: &DependencyGraph = project.graph().ok_or("expected graph")?;
    let source_map: &SourceMap = project.source_map();

    assert_eq!(canonical.name.as_str(), "valid-minimal");
    assert!(
        selection
            .active_services
            .iter()
            .any(|service| service.as_str() == "web")
    );
    assert_eq!(graph.order.len(), 1);
    assert!(!susun::render_diagnostics(project.diagnostics(), source_map).contains("error["));
    Ok(())
}

#[test]
fn workspace_env_var_overrides_compose_project_name() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path())
        .with_env_var("COMPOSE_PROJECT_NAME", "from-sdk-env")
        .analyze()?;
    let summary = project.summary();

    assert_eq!(summary.project_name.as_deref(), Some("from-sdk-env"));
    assert_eq!(
        project.identity().ok_or("expected identity")?.name.as_str(),
        "from-sdk-env"
    );
    Ok(())
}

#[test]
fn workspace_env_vars_replace_process_environment_for_sdk_analysis() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path())
        .with_env_vars([("COMPOSE_PROJECT_NAME", "from-sdk-map")])
        .analyze()?;

    assert_eq!(
        project.summary().project_name.as_deref(),
        Some("from-sdk-map")
    );
    Ok(())
}

#[test]
fn workspace_diagnostics_helpers_render_analysis_report() -> TestResult {
    let project = SusunWorkspace::from_file(malformed_path()).analyze()?;

    assert!(project.has_errors());
    assert!(project.diagnostic_count() > 0);
    assert!(project.diagnostics().has_errors());

    let text = project.render_diagnostics();
    let json = project.render_diagnostics_json();

    assert!(text.contains("error["));
    assert!(json.contains("\"diagnostics\""));
    assert!(json.contains("\"severity\""));
    Ok(())
}

#[test]
fn workspace_diagnostics_summary_json_helpers_roundtrip() -> TestResult {
    let project = SusunWorkspace::from_file(malformed_path()).analyze()?;
    let summary = project.diagnostics_summary();

    assert_eq!(
        summary.schema_version,
        DiagnosticReportSummarySchemaVersion::CURRENT
    );
    assert!(summary.has_errors);
    assert_eq!(summary.diagnostic_count, summary.diagnostics.len());
    assert!(summary.diagnostic_count > 0);

    let json = render_diagnostic_report_summary_json(&summary)?;
    let parsed = parse_diagnostic_report_summary_json(&json)?;

    assert_eq!(parsed, summary);
    assert_eq!(project.render_diagnostics_summary_json()?, json);
    Ok(())
}

#[test]
fn diagnostics_summary_json_helper_rejects_unsupported_schema_version() -> TestResult {
    let project = SusunWorkspace::from_file(malformed_path()).analyze()?;
    let summary = project.diagnostics_summary();
    let json = render_diagnostic_report_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["schema_version"]["minor"] = serde_json::json!(1);

    let result = parse_diagnostic_report_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn diagnostics_summary_json_helper_rejects_inconsistent_counts() -> TestResult {
    let project = SusunWorkspace::from_file(malformed_path()).analyze()?;
    let summary = project.diagnostics_summary();
    let json = render_diagnostic_report_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["diagnostic_count"] = serde_json::json!(999);

    let result = parse_diagnostic_report_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn diagnostics_summary_json_helper_rejects_inconsistent_error_flag() -> TestResult {
    let project = SusunWorkspace::from_file(malformed_path()).analyze()?;
    let summary = project.diagnostics_summary();
    let json = render_diagnostic_report_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["has_errors"] = serde_json::json!(false);

    let result = parse_diagnostic_report_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn sdk_project_into_analysis_returns_owned_analysis() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let analysis = project.into_analysis();

    assert!(analysis.project.is_some());
    assert!(analysis.selection.is_some());
    assert!(analysis.graph.is_some());
    Ok(())
}

#[test]
fn sdk_project_into_parts_preserves_workspace_and_identity() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path())
        .with_project_name("parts-app")
        .analyze()?;

    let (workspace, analysis, identity) = project.into_parts();

    assert_eq!(workspace.project_name(), Some("parts-app"));
    assert!(analysis.project.is_some());
    assert_eq!(
        identity.ok_or("expected identity")?.name.as_str(),
        "parts-app"
    );
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
fn facade_execution_plan_json_helper_rejects_unsupported_schema_version() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let plan = project
        .dry_run_up_plan(false)?
        .ok_or("expected daemon-free up plan")?;
    let json = render_execution_plan_json(&plan)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["schema_version"]["minor"] = serde_json::json!(1);

    let result = parse_execution_plan_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn facade_plan_outcome_summary_json_helpers_roundtrip() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let outcome = project.dry_run_up(false)?;
    let summary = PlanOutcomeSummary::from(&outcome);

    assert_eq!(
        summary.schema_version,
        PlanOutcomeSummarySchemaVersion::CURRENT
    );
    assert!(summary.planned);
    assert!(summary.plan_id.is_some());
    assert_eq!(summary.operation.as_deref(), Some("up"));
    assert!(summary.action_count > 0);
    assert!(!summary.has_errors);

    let json = render_plan_outcome_summary_json(&summary)?;
    let parsed = parse_plan_outcome_summary_json(&json)?;

    assert_eq!(parsed, summary);
    Ok(())
}

#[test]
fn plan_outcome_summary_json_helper_rejects_unsupported_schema_version() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let outcome = project.dry_run_up(false)?;
    let summary = PlanOutcomeSummary::from(&outcome);
    let json = render_plan_outcome_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["schema_version"]["minor"] = serde_json::json!(1);

    let result = parse_plan_outcome_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn plan_outcome_summary_json_helper_rejects_inconsistent_counts() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let outcome = project.dry_run_up(false)?;
    let summary = PlanOutcomeSummary::from(&outcome);
    let json = render_plan_outcome_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["action_count"] = serde_json::json!(999);

    let result = parse_plan_outcome_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn facade_plan_outcome_summary_covers_blocked_outcomes() -> TestResult {
    let project = SusunWorkspace::from_file(malformed_path()).analyze()?;
    let outcome = project.dry_run_up(false)?;
    let summary = PlanOutcomeSummary::from(&outcome);

    assert!(!summary.planned);
    assert_eq!(summary.plan_id, None);
    assert_eq!(summary.operation, None);
    assert_eq!(summary.action_count, 0);
    assert!(summary.has_errors);
    assert!(summary.diagnostic_count > 0);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SUS-PLAN-100")
    );
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
fn facade_execution_report_json_helper_rejects_inconsistent_summary() -> TestResult {
    let project = SusunWorkspace::from_file(valid_path()).analyze()?;
    let plan = project
        .dry_run_up_plan(false)?
        .ok_or("expected daemon-free up plan")?;
    let report = ExecutionReport::pending(&plan);
    let json = render_execution_report_json(&report)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["summary"]["total_actions"] = serde_json::json!(999);

    let result = parse_execution_report_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
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
fn facade_runtime_profile_summary_json_is_redacted() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::new(
            susun::EngineConnectionProfileId::new("private")?,
            susun::EngineConnectionDisplayName::new("Private Socket")?,
            susun::EngineEndpoint::UnixSocket("/very/private/docker.sock".into()),
        )
        .with_default(true),
    ])?;
    let summary = EngineConnectionProfileSetSummary::from(&set);

    assert_eq!(
        summary.schema_version,
        EngineConnectionProfileSetSummarySchemaVersion::CURRENT
    );
    assert_eq!(summary.default_profile_id.as_deref(), Some("private"));
    assert_eq!(summary.profiles[0].id, "private");
    assert_eq!(summary.profiles[0].display_name, "Private Socket");
    assert_eq!(
        summary.profiles[0].endpoint_kind,
        susun::EngineEndpointKind::UnixSocket
    );
    assert_eq!(
        summary.profiles[0].redacted_endpoint.to_string(),
        "unix://<local-socket>"
    );

    let json = render_engine_connection_profile_set_summary_json(&summary)?;
    assert!(!json.contains("very/private"));
    assert!(!json.contains("docker.sock"));

    let parsed = parse_engine_connection_profile_set_summary_json(&json)?;
    assert_eq!(parsed, summary);
    Ok(())
}

#[test]
fn facade_runtime_profile_summary_json_rejects_unsupported_schema_version() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::local_default(),
    ])?;
    let summary = EngineConnectionProfileSetSummary::from(&set);
    let json = render_engine_connection_profile_set_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["schema_version"]["minor"] = serde_json::json!(1);

    let result = parse_engine_connection_profile_set_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn facade_runtime_profile_summary_json_rejects_inconsistent_default() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::local_default(),
    ])?;
    let summary = EngineConnectionProfileSetSummary::from(&set);
    let json = render_engine_connection_profile_set_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["default_profile_id"] = serde_json::json!("missing");

    let result = parse_engine_connection_profile_set_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn facade_runtime_profile_summary_json_rejects_invalid_identity_fields() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::local_default(),
    ])?;
    let summary = EngineConnectionProfileSetSummary::from(&set);
    let json = render_engine_connection_profile_set_summary_json(&summary)?;

    let mut bad_id: serde_json::Value = serde_json::from_str(&json)?;
    bad_id["profiles"][0]["id"] = serde_json::json!("bad id");
    assert!(
        parse_engine_connection_profile_set_summary_json(&serde_json::to_string(&bad_id)?).is_err()
    );

    let mut bad_name: serde_json::Value = serde_json::from_str(&json)?;
    bad_name["profiles"][0]["display_name"] = serde_json::json!("  Local runtime  ");
    assert!(
        parse_engine_connection_profile_set_summary_json(&serde_json::to_string(&bad_name)?)
            .is_err()
    );
    Ok(())
}

#[test]
fn facade_runtime_profile_summary_json_rejects_endpoint_kind_drift() -> TestResult {
    let set = susun::EngineConnectionProfileSet::new(vec![
        susun::EngineConnectionProfile::local_default(),
    ])?;
    let summary = EngineConnectionProfileSetSummary::from(&set);
    let json = render_engine_connection_profile_set_summary_json(&summary)?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    value["profiles"][0]["endpoint_kind"] = serde_json::json!("tcp");

    let result = parse_engine_connection_profile_set_summary_json(&serde_json::to_string(&value)?);

    assert!(result.is_err());
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
