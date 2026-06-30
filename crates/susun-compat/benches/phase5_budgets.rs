//! Cross-phase performance baseline and regression report harness.

use std::{
    hint::black_box,
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime},
};

use susun::{Analyzer, Planner};
use susun_build::{BuildInputManifest, Dockerignore};
use susun_compat::{BenchmarkSample, BenchmarkUnit, PerformanceBudgetManifest, PerformanceReport};
use susun_convergence::{
    ConvergenceInput, ConvergencePolicy, DesiredDeployment, DesiredInstanceFingerprints,
    ObservedDeployment, plan_convergence,
};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity, ProjectInstanceId};
use susun_planner::UpPlanOptions;

const BUDGETS_JSON: &str = include_str!("../../../fixtures/compatibility/performance-budgets.json");

fn main() {
    let manifest = match PerformanceBudgetManifest::from_json_str(BUDGETS_JSON) {
        Ok(manifest) => manifest,
        Err(error) => exit_with_error("failed to parse performance budgets", error),
    };
    let workspace = workspace_root();
    let samples = vec![
        measure("analysis", 20, || analysis(&workspace)),
        measure("planning", 50, || planning(&workspace)),
        measure("snapshot_indexing", 1_000, || snapshot_indexing(&workspace)),
        measure("convergence", 500, || convergence(&workspace)),
        measure("build_context_enumeration", 50, || {
            build_context_enumeration(&workspace)
        }),
        measure("cli_startup", 3, || cli_startup(&workspace)),
    ];

    let report = match PerformanceReport::from_samples(
        env!("CARGO_PKG_VERSION"),
        runner_name(),
        &manifest,
        samples,
    ) {
        Ok(report) => report,
        Err(error) => exit_with_error("failed to build performance report", error),
    };
    let output = match serde_json::to_string_pretty(&report) {
        Ok(output) => output,
        Err(error) => exit_with_error("failed to serialize performance report", error),
    };
    println!("{output}");
}

fn measure(name: &'static str, iterations: u32, mut operation: impl FnMut()) -> BenchmarkSample {
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    let elapsed = started.elapsed().as_micros();
    BenchmarkSample::new(
        name,
        iterations,
        BenchmarkUnit::Microseconds,
        elapsed / u128::from(iterations),
    )
}

fn analysis(workspace: &Path) {
    let result =
        match Analyzer::new(workspace.join("fixtures/cli/valid-minimal/compose.yaml")).analyze() {
            Ok(result) => result,
            Err(error) => exit_with_error("analysis benchmark failed", error),
        };
    black_box(result.report.has_errors());
}

fn planning(workspace: &Path) {
    let analysis =
        match Analyzer::new(workspace.join("fixtures/cli/valid-minimal/compose.yaml")).analyze() {
            Ok(result) => result,
            Err(error) => exit_with_error("planning analysis setup failed", error),
        };
    let Some(project) = analysis.project.as_ref() else {
        exit_with_message("planning benchmark did not produce a project");
    };
    let identity = ProjectIdentity::new(
        project.name.clone(),
        ProjectInstanceId::derive(&project.name, workspace),
    );
    let planner = Planner::new(
        identity,
        EngineCapabilities::permissive_local(),
        EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
    );
    let outcome = match planner.plan_up(&analysis, UpPlanOptions::default()) {
        Ok(outcome) => outcome,
        Err(error) => exit_with_error("planning benchmark failed", error),
    };
    black_box(outcome.diagnostics.has_errors());
}

fn snapshot_indexing(workspace: &Path) {
    let analysis =
        match Analyzer::new(workspace.join("fixtures/cli/valid-minimal/compose.yaml")).analyze() {
            Ok(result) => result,
            Err(error) => exit_with_error("snapshot indexing setup failed", error),
        };
    let Some(selection) = analysis.selection.as_ref() else {
        exit_with_message("snapshot indexing benchmark did not produce a selection");
    };
    let observed = ObservedDeployment::new(
        EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
        &selection.active_services,
    );
    black_box(observed.ownership.containers.len());
}

fn convergence(workspace: &Path) {
    let analysis =
        match Analyzer::new(workspace.join("fixtures/cli/valid-minimal/compose.yaml")).analyze() {
            Ok(result) => result,
            Err(error) => exit_with_error("convergence setup failed", error),
        };
    let Some(project) = analysis.project.clone() else {
        exit_with_message("convergence benchmark did not produce a project");
    };
    let Some(selection) = analysis.selection.clone() else {
        exit_with_message("convergence benchmark did not produce a selection");
    };
    let Some(graph) = analysis.graph.clone() else {
        exit_with_message("convergence benchmark did not produce a graph");
    };
    let identity = ProjectIdentity::new(
        project.name.clone(),
        ProjectInstanceId::derive(&project.name, workspace),
    );
    let desired = DesiredDeployment::new(
        project,
        selection.clone(),
        graph,
        identity,
        Default::default(),
    );
    let observed = ObservedDeployment::new(
        EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
        &selection.active_services,
    );
    let policy = ConvergencePolicy::default();
    let fingerprints = DesiredInstanceFingerprints::default();
    let outcome = match plan_convergence(ConvergenceInput {
        desired: &desired,
        observed: &observed,
        capabilities: &EngineCapabilities::permissive_local(),
        policy: &policy,
        desired_fingerprints: &fingerprints,
    }) {
        Ok(outcome) => outcome,
        Err(error) => exit_with_error("convergence benchmark failed", error),
    };
    black_box(outcome.decisions.len());
}

fn build_context_enumeration(workspace: &Path) {
    let manifest = match BuildInputManifest::from_context(
        &workspace.join("fixtures/compatibility/build-context"),
        &Dockerignore::default(),
    ) {
        Ok(manifest) => manifest,
        Err(error) => exit_with_error("build context benchmark failed", error),
    };
    black_box(manifest.entries.len());
}

fn cli_startup(workspace: &Path) {
    let output = match Command::new(cli_path(workspace)).arg("--help").output() {
        Ok(output) => output,
        Err(error) => exit_with_error("cli startup benchmark failed", error),
    };
    if !output.status.success() {
        exit_with_message("cli startup benchmark command failed");
    }
    black_box(output.stdout.len());
}

fn cli_path(workspace: &Path) -> PathBuf {
    if let Some(path) = std::env::var_os("SUSUN_CLI_PATH") {
        return PathBuf::from(path);
    }
    workspace
        .join("target")
        .join("debug")
        .join(format!("susun{}", std::env::consts::EXE_SUFFIX))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn runner_name() -> String {
    std::env::var("SUSUN_PERF_RUNNER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "local".to_owned())
}

fn exit_with_error(context: &str, error: impl std::fmt::Display) -> ! {
    eprintln!("{context}: {error}");
    std::process::exit(2);
}

fn exit_with_message(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
