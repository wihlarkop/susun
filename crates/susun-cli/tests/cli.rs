use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn valid_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/cli/valid-minimal/compose.yaml")
}

fn malformed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cli/malformed/compose.yaml")
}

fn susun() -> Command {
    Command::cargo_bin("susun").expect("susun binary not found")
}

#[test]
fn check_valid_file_exits_0() {
    susun().args(["check", valid_path().to_str().expect("valid path")]).assert().success();
}

#[test]
fn check_malformed_file_exits_1() {
    susun()
        .args(["check", malformed_path().to_str().expect("malformed path")])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("SUS-PARSE-001"));
}

#[test]
fn check_missing_file_exits_2() {
    susun()
        .args(["check", "/nonexistent/compose.yaml"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn config_valid_file_prints_json_and_exits_0() {
    susun()
        .args(["config", valid_path().to_str().expect("valid path")])
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
        .args(["config", malformed_path().to_str().expect("malformed path")])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn config_missing_file_exits_2() {
    susun()
        .args(["config", "/nonexistent/compose.yaml"])
        .assert()
        .failure()
        .code(2);
}
