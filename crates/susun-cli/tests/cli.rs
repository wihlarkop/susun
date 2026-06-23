use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn susun() -> Command {
    Command::cargo_bin("susun").expect("susun binary not found")
}

// ── check subcommand ──────────────────────────────────────────────────────────

#[test]
fn check_valid_file_exits_0() {
    susun()
        .args(["-f", fixture("cli/valid-minimal/compose.yaml").to_str().expect("valid path")])
        .arg("check")
        .assert()
        .success();
}

#[test]
fn check_malformed_file_exits_1() {
    susun()
        .args(["-f", fixture("cli/malformed/compose.yaml").to_str().expect("malformed path")])
        .arg("check")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("SUS-PARSE-001"));
}

#[test]
fn check_missing_file_exits_2() {
    susun()
        .args(["-f", "/nonexistent/compose.yaml"])
        .arg("check")
        .assert()
        .failure()
        .code(2);
}

// ── config subcommand ─────────────────────────────────────────────────────────

#[test]
fn config_valid_file_prints_json_and_exits_0() {
    susun()
        .args(["-f", fixture("cli/valid-minimal/compose.yaml").to_str().expect("valid path")])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("valid-minimal"))
        .stdout(predicate::str::contains("\"services\""))
        .stdout(predicate::str::contains("nginx:latest"));
}

#[test]
fn config_malformed_file_exits_1() {
    susun()
        .args(["-f", fixture("cli/malformed/compose.yaml").to_str().expect("malformed path")])
        .arg("config")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn config_missing_file_exits_2() {
    susun()
        .args(["-f", "/nonexistent/compose.yaml"])
        .arg("config")
        .assert()
        .failure()
        .code(2);
}

// ── --project-name / -p ───────────────────────────────────────────────────────

#[test]
fn project_name_flag_overrides_name_field() {
    susun()
        .args(["-f", fixture("cli/valid-minimal/compose.yaml").to_str().expect("valid path")])
        .args(["-p", "my-override"])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"my-override\""));
}

#[test]
fn project_name_long_form_works() {
    susun()
        .args(["-f", fixture("cli/valid-minimal/compose.yaml").to_str().expect("valid path")])
        .args(["--project-name", "long-form"])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"long-form\""));
}

// ── --env-file ────────────────────────────────────────────────────────────────

#[test]
fn env_file_substitutes_project_name() {
    let compose = fixture("cli/with-env-file/compose.yaml");
    let env = fixture("cli/with-env-file/explicit.env");
    susun()
        .args(["-f", compose.to_str().expect("compose path")])
        .args(["--env-file", env.to_str().expect("env path")])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("from-explicit-env-file"));
}

#[test]
fn default_dotenv_is_loaded_automatically() {
    // compose.yaml is in a directory that also has a .env file.
    // The .env should be auto-loaded.
    let compose = fixture("cli/with-env-file/compose.yaml");
    susun()
        .args(["-f", compose.to_str().expect("compose path")])
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("from-dotenv"));
}

#[test]
fn missing_explicit_env_file_exits_2() {
    let compose = fixture("cli/valid-minimal/compose.yaml");
    susun()
        .args(["-f", compose.to_str().expect("compose path")])
        .args(["--env-file", "/nonexistent/.env"])
        .arg("config")
        .assert()
        .failure()
        .code(2);
}

// ── --profile ─────────────────────────────────────────────────────────────────

#[test]
fn profile_flag_is_accepted_without_error() {
    // Profile selection is fully wired in Task 27; for now verify the flag is
    // parsed without error and analysis completes normally.
    susun()
        .args(["-f", fixture("cli/valid-minimal/compose.yaml").to_str().expect("valid path")])
        .args(["--profile", "debug"])
        .arg("check")
        .assert()
        .success();
}
