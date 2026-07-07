#![allow(missing_docs)]

use std::{error::Error, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

fn fixture(rel: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
        .display()
        .to_string()
}

fn susun() -> Result<Command, Box<dyn Error>> {
    Ok(Command::cargo_bin("susun")?)
}

// ── check subcommand ──────────────────────────────────────────────────────────

#[test]
fn help_lists_doctor_command() -> TestResult {
    susun()?
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("doctor"));
    Ok(())
}

#[test]
fn check_valid_file_exits_0() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .arg("check")
        .assert()
        .success();
    Ok(())
}

#[test]
fn check_malformed_file_exits_1() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/malformed/compose.yaml")])
        .arg("check")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("SUS-PARSE-001"));
    Ok(())
}

#[test]
fn check_missing_file_exits_2() -> TestResult {
    susun()?
        .args(["-f", "/nonexistent/compose.yaml"])
        .arg("check")
        .assert()
        .failure()
        .code(2);
    Ok(())
}

// ── config subcommand ─────────────────────────────────────────────────────────

#[test]
fn config_valid_file_prints_json_and_exits_0() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("valid-minimal"))
        .stdout(predicate::str::contains("\"services\""))
        .stdout(predicate::str::contains("nginx:latest"));
    Ok(())
}

#[test]
fn config_malformed_file_exits_1() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/malformed/compose.yaml")])
        .arg("config")
        .assert()
        .failure()
        .code(1);
    Ok(())
}

#[test]
fn config_missing_file_exits_2() -> TestResult {
    susun()?
        .args(["-f", "/nonexistent/compose.yaml"])
        .arg("config")
        .assert()
        .failure()
        .code(2);
    Ok(())
}

#[test]
fn summary_json_prints_sdk_project_summary() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .args(["--format", "json"])
        .arg("summary")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\""))
        .stdout(predicate::str::contains("\"major\": 1"))
        .stdout(predicate::str::contains("\"project_name\""))
        .stdout(predicate::str::contains("valid-minimal"))
        .stdout(predicate::str::contains("\"service_count\": 1"))
        .stdout(predicate::str::contains("\"name\": \"web\""));
    Ok(())
}

#[test]
fn summary_json_prints_resource_details_without_secret_values() -> TestResult {
    susun()?
        .args([
            "-f",
            &fixture("compatibility/resources-configs-secrets/compose.yaml"),
        ])
        .args(["--format", "json"])
        .arg("summary")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"configs\""))
        .stdout(predicate::str::contains("app_config"))
        .stdout(predicate::str::contains("\"secrets\""))
        .stdout(predicate::str::contains("app_secret"))
        .stdout(predicate::str::contains("super-secret").not());
    Ok(())
}

// ── --project-name / -p ───────────────────────────────────────────────────────

#[test]
fn doctor_json_emits_runtime_report_shape() -> TestResult {
    susun()?
        .args(["--format", "json"])
        .arg("doctor")
        .assert()
        .stdout(predicate::str::contains("\"status\""))
        .stdout(predicate::str::contains("\"endpoint\""));
    Ok(())
}

#[test]
fn project_name_flag_overrides_name_field() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .args(["-p", "my-override"])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"my-override\""));
    Ok(())
}

#[test]
fn project_name_long_form_works() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .args(["--project-name", "long-form"])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"long-form\""));
    Ok(())
}

// ── --env-file ────────────────────────────────────────────────────────────────

#[test]
fn env_file_substitutes_project_name() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/with-env-file/compose.yaml")])
        .args(["--env-file", &fixture("cli/with-env-file/explicit.env")])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("from-explicit-env-file"));
    Ok(())
}

#[test]
fn default_dotenv_is_loaded_automatically() -> TestResult {
    // compose.yaml is in a directory that also has a .env file.
    // The .env should be auto-loaded.
    susun()?
        .args(["-f", &fixture("cli/with-env-file/compose.yaml")])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("from-dotenv"));
    Ok(())
}

#[test]
fn missing_explicit_env_file_exits_2() -> TestResult {
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .args(["--env-file", "/nonexistent/.env"])
        .arg("config")
        .assert()
        .failure()
        .code(2);
    Ok(())
}

// ── --profile ─────────────────────────────────────────────────────────────────

#[test]
fn profile_flag_is_accepted_without_error() -> TestResult {
    // Profile selection is fully wired in Task 27; for now verify the flag is
    // parsed without error and analysis completes normally.
    susun()?
        .args(["-f", &fixture("cli/valid-minimal/compose.yaml")])
        .args(["--profile", "debug"])
        .arg("check")
        .assert()
        .success();
    Ok(())
}
