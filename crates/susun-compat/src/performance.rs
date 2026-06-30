//! Cross-phase performance budgets and regression report types.

use crate::CompatibilityError;

/// Schema version for performance budget and report artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PerformanceSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl PerformanceSchemaVersion {
    /// Current performance schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

impl Default for PerformanceSchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

/// Unit used by a benchmark measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum BenchmarkUnit {
    /// Milliseconds.
    Milliseconds,
    /// Microseconds.
    Microseconds,
}

/// One required performance budget.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PerformanceBudget {
    /// Stable benchmark identifier.
    pub name: String,
    /// Expected sample unit.
    pub unit: BenchmarkUnit,
    /// Maximum allowed value for a single reported sample.
    pub max: u128,
    /// Human-readable purpose or scope for release docs.
    pub description: String,
}

/// Versioned performance budget manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PerformanceBudgetManifest {
    /// Manifest schema version.
    pub schema_version: PerformanceSchemaVersion,
    /// Human-readable scope.
    pub scope: String,
    /// Required benchmark budgets.
    pub budgets: Vec<PerformanceBudget>,
    /// Notes for release docs.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub notes: Vec<String>,
}

impl PerformanceBudgetManifest {
    /// Parses a performance budget manifest from JSON.
    #[cfg(feature = "serde")]
    pub fn from_json_str(input: &str) -> Result<Self, CompatibilityError> {
        let manifest: Self = serde_json::from_str(input)?;
        if manifest.budgets.is_empty() {
            return Err(CompatibilityError::EmptyPerformanceBudgets);
        }
        Ok(manifest)
    }
}

/// One benchmark measurement.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BenchmarkSample {
    /// Stable benchmark identifier.
    pub name: String,
    /// Number of iterations represented by this sample.
    pub iterations: u32,
    /// Sample unit.
    pub unit: BenchmarkUnit,
    /// Measured value in the sample unit.
    pub value: u128,
}

impl BenchmarkSample {
    /// Creates a benchmark sample.
    #[must_use]
    pub fn new(name: impl Into<String>, iterations: u32, unit: BenchmarkUnit, value: u128) -> Self {
        Self {
            name: name.into(),
            iterations,
            unit,
            value,
        }
    }
}

/// Regression status for a report or individual result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PerformanceStatus {
    /// All reported samples were within their documented budgets.
    WithinBudget,
    /// At least one reported sample exceeded its documented budget.
    Regression,
}

/// One benchmark result evaluated against a budget.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PerformanceReportResult {
    /// Stable benchmark identifier.
    pub name: String,
    /// Expected sample unit.
    pub unit: BenchmarkUnit,
    /// Maximum allowed value.
    pub budget: u128,
    /// Measured value.
    pub measured: u128,
    /// Number of iterations represented by the sample.
    pub iterations: u32,
    /// Per-benchmark status.
    pub status: PerformanceStatus,
}

/// Versioned benchmark regression report.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PerformanceReport {
    /// Report schema version.
    pub schema_version: PerformanceSchemaVersion,
    /// Susun version this report describes.
    pub susun_version: String,
    /// Runner identity used for comparison context.
    pub runner: String,
    /// Overall report status.
    pub status: PerformanceStatus,
    /// Deterministically ordered results, matching the budget manifest order.
    pub results: Vec<PerformanceReportResult>,
}

impl PerformanceReport {
    /// Builds a report from benchmark samples and a budget manifest.
    pub fn from_samples(
        susun_version: impl Into<String>,
        runner: impl Into<String>,
        manifest: &PerformanceBudgetManifest,
        samples: Vec<BenchmarkSample>,
    ) -> Result<Self, CompatibilityError> {
        let mut results = Vec::with_capacity(manifest.budgets.len());
        let mut status = PerformanceStatus::WithinBudget;

        for budget in &manifest.budgets {
            let sample = samples
                .iter()
                .find(|sample| sample.name == budget.name)
                .ok_or_else(|| CompatibilityError::MissingPerformanceSample {
                    name: budget.name.clone(),
                })?;
            let result_status = if sample.unit == budget.unit && sample.value <= budget.max {
                PerformanceStatus::WithinBudget
            } else {
                PerformanceStatus::Regression
            };
            if result_status == PerformanceStatus::Regression {
                status = PerformanceStatus::Regression;
            }
            results.push(PerformanceReportResult {
                name: budget.name.clone(),
                unit: budget.unit,
                budget: budget.max,
                measured: sample.value,
                iterations: sample.iterations,
                status: result_status,
            });
        }

        Ok(Self {
            schema_version: PerformanceSchemaVersion::CURRENT,
            susun_version: susun_version.into(),
            runner: runner.into(),
            status,
            results,
        })
    }
}
