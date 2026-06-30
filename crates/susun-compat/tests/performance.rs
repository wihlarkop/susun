//! Performance budget and report contract tests.

#![cfg(feature = "serde")]

use susun_compat::{
    BenchmarkSample, BenchmarkUnit, PerformanceBudgetManifest, PerformanceReport, PerformanceStatus,
};

const BUDGETS_JSON: &str = include_str!("../../../fixtures/compatibility/performance-budgets.json");

#[test]
fn parses_cross_phase_performance_budget_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PerformanceBudgetManifest::from_json_str(BUDGETS_JSON)?;

    assert_eq!(manifest.scope, "cross-phase-performance");
    assert_eq!(manifest.budgets.len(), 6);
    assert_eq!(
        manifest
            .budgets
            .iter()
            .map(|budget| budget.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "analysis",
            "planning",
            "snapshot_indexing",
            "convergence",
            "build_context_enumeration",
            "cli_startup",
        ]
    );
    assert!(
        manifest
            .budgets
            .iter()
            .all(|budget| budget.unit == BenchmarkUnit::Microseconds)
    );
    Ok(())
}

#[test]
fn report_marks_samples_against_budget_thresholds() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PerformanceBudgetManifest::from_json_str(BUDGETS_JSON)?;

    let report = PerformanceReport::from_samples(
        "0.1.0",
        "local-dev",
        &manifest,
        vec![
            BenchmarkSample::new("analysis", 1, BenchmarkUnit::Microseconds, 2_500),
            BenchmarkSample::new("planning", 1, BenchmarkUnit::Microseconds, 1_500),
            BenchmarkSample::new("snapshot_indexing", 1, BenchmarkUnit::Microseconds, 700),
            BenchmarkSample::new("convergence", 1, BenchmarkUnit::Microseconds, 2_000),
            BenchmarkSample::new(
                "build_context_enumeration",
                1,
                BenchmarkUnit::Microseconds,
                3_000,
            ),
            BenchmarkSample::new("cli_startup", 1, BenchmarkUnit::Microseconds, 80_000),
        ],
    )?;

    assert_eq!(report.status, PerformanceStatus::WithinBudget);
    assert!(
        report
            .results
            .iter()
            .all(|result| result.status == PerformanceStatus::WithinBudget)
    );
    Ok(())
}

#[test]
fn report_flags_regressions_and_missing_benchmarks() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PerformanceBudgetManifest::from_json_str(BUDGETS_JSON)?;

    let regression = PerformanceReport::from_samples(
        "0.1.0",
        "local-dev",
        &manifest,
        vec![
            BenchmarkSample::new("analysis", 1, BenchmarkUnit::Microseconds, 250_000),
            BenchmarkSample::new("planning", 1, BenchmarkUnit::Microseconds, 1_500),
            BenchmarkSample::new("snapshot_indexing", 1, BenchmarkUnit::Microseconds, 700),
            BenchmarkSample::new("convergence", 1, BenchmarkUnit::Microseconds, 2_000),
            BenchmarkSample::new(
                "build_context_enumeration",
                1,
                BenchmarkUnit::Microseconds,
                3_000,
            ),
            BenchmarkSample::new("cli_startup", 1, BenchmarkUnit::Microseconds, 80_000),
        ],
    )?;
    assert_eq!(regression.status, PerformanceStatus::Regression);
    assert_eq!(regression.results[0].status, PerformanceStatus::Regression);

    let missing = PerformanceReport::from_samples(
        "0.1.0",
        "local-dev",
        &manifest,
        vec![BenchmarkSample::new(
            "analysis",
            1,
            BenchmarkUnit::Microseconds,
            2_500,
        )],
    );
    let Err(missing) = missing else {
        return Err("expected missing benchmark error".into());
    };
    assert!(
        missing
            .to_string()
            .contains("missing performance sample for planning")
    );
    Ok(())
}
