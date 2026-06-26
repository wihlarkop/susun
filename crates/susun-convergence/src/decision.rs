//! Convergence difference and decision types.

use susun_engine::ServiceInstanceId;

/// Runtime drift observed for a compatible container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RuntimeDrift {
    /// Container is running but needs a restart to match runtime policy.
    NeedsRestart,
    /// Container is paused.
    Paused,
    /// Container is restarting.
    Restarting,
}

/// Fingerprint or desired-state field that changed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ChangedField(String);

impl ChangedField {
    /// Creates a changed-field key.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the field key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Difference between one desired service instance and observed state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum InstanceDifference {
    /// Desired instance has no observed container.
    Missing,
    /// Observed container is compatible and running.
    Unchanged,
    /// Observed container is compatible but stopped.
    StoppedButCompatible,
    /// Image identity changed.
    ImageChanged,
    /// Configuration changed.
    ConfigurationChanged {
        /// Changed fields.
        fields: Vec<ChangedField>,
    },
    /// Runtime state drift.
    RuntimeStateDrift {
        /// Drift kind.
        drift: RuntimeDrift,
    },
    /// Multiple resources claim the same desired identity.
    OwnershipAmbiguous,
    /// Observed state cannot be safely interpreted.
    UnsupportedObservedState,
}

/// High-level convergence decision kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ConvergenceDecisionKind {
    /// No operation is needed.
    NoOp,
    /// Create the missing instance.
    Create,
    /// Start a compatible stopped instance.
    Start,
    /// Restart an existing instance.
    Restart,
    /// Recreate the instance.
    Recreate,
    /// Stop the instance.
    Stop,
    /// Remove an orphaned or scaled-down instance.
    Remove,
    /// Block due to unsafe or ambiguous state.
    Block,
}

/// Human-readable but deterministic reason for a convergence decision.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecisionReason {
    /// Stable reason code.
    pub code: String,
    /// Redacted summary.
    pub summary: String,
}

impl DecisionReason {
    /// Creates a decision reason.
    pub fn new(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            summary: summary.into(),
        }
    }
}

/// Decision for one service instance.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConvergenceDecision {
    /// Service instance.
    pub instance: ServiceInstanceId,
    /// Observed difference.
    pub difference: InstanceDifference,
    /// Chosen decision.
    pub kind: ConvergenceDecisionKind,
    /// Explainable reasons.
    pub reasons: Vec<DecisionReason>,
    /// Whether the decision can mutate engine state.
    pub mutates: bool,
    /// Whether downtime is expected.
    pub downtime_expected: bool,
    /// Whether the decision is destructive.
    pub destructive: bool,
}

impl ConvergenceDecision {
    /// Creates a convergence decision from its core fields.
    pub fn new(
        instance: ServiceInstanceId,
        difference: InstanceDifference,
        kind: ConvergenceDecisionKind,
    ) -> Self {
        let mutates = !matches!(
            kind,
            ConvergenceDecisionKind::NoOp | ConvergenceDecisionKind::Block
        );
        let destructive = matches!(
            kind,
            ConvergenceDecisionKind::Remove | ConvergenceDecisionKind::Recreate
        );
        let downtime_expected = matches!(
            kind,
            ConvergenceDecisionKind::Restart
                | ConvergenceDecisionKind::Recreate
                | ConvergenceDecisionKind::Stop
                | ConvergenceDecisionKind::Remove
        );
        Self {
            instance,
            difference,
            kind,
            reasons: Vec::new(),
            mutates,
            downtime_expected,
            destructive,
        }
    }

    /// Adds an explainable reason.
    pub fn with_reason(mut self, reason: DecisionReason) -> Self {
        self.reasons.push(reason);
        self
    }
}
