//! Compatibility corpus manifest types.

use std::path::PathBuf;

use crate::{
    CompatibilityError, ComposeReference, FixtureConfig, OracleCommand, OracleConfig,
    OracleOperation, OracleSchemaVersion,
};

/// Schema version for compatibility corpus manifests.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CorpusSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl CorpusSchemaVersion {
    /// Current corpus schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

impl Default for CorpusSchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

/// Secret handling assertion for a compatibility fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SecretHygiene {
    /// The fixture does not contain secret material.
    NoSecrets,
    /// The fixture contains only non-sensitive placeholder values.
    SyntheticOnly,
}

/// Coverage area represented by a corpus fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CorpusCoverage {
    /// Compose analysis and diagnostics.
    Analysis,
    /// Canonical config output.
    Config,
    /// Execution planning.
    Plan,
    /// Include graph handling.
    Include,
    /// Service inheritance handling.
    Extends,
    /// Advanced merge tag behavior.
    MergeTags,
    /// Build model and context behavior.
    Build,
    /// Config resource behavior.
    Configs,
    /// Secret resource behavior.
    Secrets,
    /// Runtime command surface.
    RuntimeCommands,
    /// Convergence planning and fingerprints.
    Convergence,
    /// Explicitly deferred coverage area.
    Deferred,
}

/// One fixture in the differential compatibility corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CorpusFixture {
    /// Stable fixture identifier.
    pub id: String,
    /// Human-readable fixture purpose.
    pub description: String,
    /// Fixture project directory, relative to the repository root.
    pub project_dir: PathBuf,
    /// Compose files passed in order.
    pub compose_files: Vec<PathBuf>,
    /// Differential operations expected for this fixture.
    pub operations: Vec<OracleOperation>,
    /// Coverage areas claimed by this fixture.
    pub coverage: Vec<CorpusCoverage>,
    /// License basis for including the fixture.
    pub license: String,
    /// Secret hygiene assertion.
    pub secret_hygiene: SecretHygiene,
    /// Coverage notes that require future PRs or external resources.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub deferred: Vec<String>,
}

impl CorpusFixture {
    /// Converts this corpus fixture into oracle fixture configuration.
    #[must_use]
    pub fn to_oracle_fixture(&self) -> FixtureConfig {
        FixtureConfig {
            id: self.id.clone(),
            project_dir: self.project_dir.clone(),
            compose_files: self.compose_files.clone(),
            operations: self.operations.clone(),
            profiles: Vec::new(),
            environment: Default::default(),
        }
    }
}

/// Top-level compatibility corpus manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CorpusManifest {
    /// Manifest schema version.
    pub schema_version: CorpusSchemaVersion,
    /// Human-readable corpus name.
    pub name: String,
    /// Included fixtures.
    pub fixtures: Vec<CorpusFixture>,
}

impl CorpusManifest {
    /// Parses a corpus manifest from JSON.
    #[cfg(feature = "serde")]
    pub fn from_json_str(input: &str) -> Result<Self, CompatibilityError> {
        let manifest: Self = serde_json::from_str(input)?;
        if manifest.fixtures.is_empty() {
            return Err(CompatibilityError::EmptyCorpus);
        }
        Ok(manifest)
    }

    /// Converts this corpus manifest to a versioned oracle configuration.
    #[must_use]
    pub fn to_oracle_config(
        &self,
        compose_reference: ComposeReference,
        command: OracleCommand,
    ) -> OracleConfig {
        OracleConfig {
            schema_version: OracleSchemaVersion::CURRENT,
            compose_reference,
            command,
            fixtures: self
                .fixtures
                .iter()
                .map(CorpusFixture::to_oracle_fixture)
                .collect(),
        }
    }
}
