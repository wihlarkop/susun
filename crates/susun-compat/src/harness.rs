//! Harness preparation for differential Compose compatibility checks.

use std::path::PathBuf;

use indexmap::IndexMap;

use crate::CompatibilityError;
use crate::oracle::{FixtureConfig, OracleConfig, OracleOperation};

/// Compatibility harness configured with a versioned oracle description.
#[derive(Clone, Debug)]
pub struct CompatibilityHarness {
    config: OracleConfig,
}

impl CompatibilityHarness {
    /// Creates a new compatibility harness after validating oracle settings.
    pub fn new(config: OracleConfig) -> Result<Self, CompatibilityError> {
        if config.command.executable.trim().is_empty() {
            return Err(CompatibilityError::EmptyOracleExecutable);
        }

        for fixture in &config.fixtures {
            validate_fixture(fixture)?;
        }

        Ok(Self { config })
    }

    /// Returns the immutable oracle configuration.
    #[must_use]
    pub fn config(&self) -> &OracleConfig {
        &self.config
    }

    /// Prepares deterministic oracle invocations for all configured fixtures.
    #[must_use]
    pub fn run_plan(&self) -> Vec<FixtureRunPlan> {
        self.config
            .fixtures
            .iter()
            .map(|fixture| FixtureRunPlan {
                fixture_id: fixture.id.clone(),
                invocations: fixture
                    .operations
                    .iter()
                    .copied()
                    .map(|operation| self.invocation_for(fixture, operation))
                    .collect(),
            })
            .collect()
    }

    fn invocation_for(
        &self,
        fixture: &FixtureConfig,
        operation: OracleOperation,
    ) -> OracleInvocation {
        let mut args = self.config.command.base_args.clone();

        for compose_file in &fixture.compose_files {
            args.push("--file".to_owned());
            args.push(compose_file.display().to_string());
        }

        for profile in &fixture.profiles {
            args.push("--profile".to_owned());
            args.push(profile.clone());
        }

        args.extend(operation.compose_args().iter().map(ToString::to_string));

        let mut environment = self.config.command.environment.clone();
        environment.extend(fixture.environment.clone());

        OracleInvocation {
            operation,
            executable: self.config.command.executable.clone(),
            args,
            environment,
            working_dir: fixture.project_dir.clone(),
        }
    }
}

/// Prepared work for a single compatibility fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FixtureRunPlan {
    /// Stable fixture identifier.
    pub fixture_id: String,
    /// Oracle commands to run for the fixture.
    pub invocations: Vec<OracleInvocation>,
}

/// Prepared external oracle command.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct OracleInvocation {
    /// Compatibility operation represented by this invocation.
    pub operation: OracleOperation,
    /// Executable name or path.
    pub executable: String,
    /// Complete command arguments.
    pub args: Vec<String>,
    /// Environment variables for the process.
    pub environment: IndexMap<String, String>,
    /// Process working directory.
    pub working_dir: PathBuf,
}

fn validate_fixture(fixture: &FixtureConfig) -> Result<(), CompatibilityError> {
    if fixture.id.trim().is_empty() {
        return Err(CompatibilityError::EmptyFixtureId);
    }

    if fixture.compose_files.is_empty() {
        return Err(CompatibilityError::MissingComposeFiles {
            fixture_id: fixture.id.clone(),
        });
    }

    for compose_file in &fixture.compose_files {
        if compose_file.is_absolute() {
            return Err(CompatibilityError::AbsoluteComposePath {
                fixture_id: fixture.id.clone(),
                path: compose_file.clone(),
            });
        }
    }

    Ok(())
}
