//! Interpolation environment precedence resolver.
//!
//! Precedence (highest → lowest):
//! 1. Process environment (or injected [`EnvironmentProvider`])
//! 2. Explicit `--env-file` entries
//! 3. Default `.env` file entries

use super::{DotenvEntry, EnvironmentProvider};

/// Resolves variable lookups across the three-layer environment stack.
///
/// Create one from a process provider and two pre-parsed [`DotenvEntry`]
/// slices, then call [`get`][EnvResolver::get] during interpolation.
pub struct EnvResolver {
    process: Box<dyn EnvironmentProvider>,
    env_file: Vec<DotenvEntry>,
    dotenv: Vec<DotenvEntry>,
}

impl EnvResolver {
    /// Creates a resolver with the given process provider and `.env` entry lists.
    ///
    /// - `process` — the process environment (or an injected mock for testing)
    /// - `env_file` — entries from the explicit `--env-file`, if any
    /// - `dotenv` — entries from the project-directory `.env`, if present
    pub fn new(
        process: impl EnvironmentProvider + 'static,
        env_file: Vec<DotenvEntry>,
        dotenv: Vec<DotenvEntry>,
    ) -> Self {
        Self { process: Box::new(process), env_file, dotenv }
    }

    /// Looks up `key` using the documented precedence order.
    ///
    /// Returns the first match found, or `None` if the key is absent in all layers.
    pub fn get(&self, key: &str) -> Option<String> {
        self.process
            .get(key)
            .or_else(|| find_in(&self.env_file, key))
            .or_else(|| find_in(&self.dotenv, key))
    }
}

fn find_in(entries: &[DotenvEntry], key: &str) -> Option<String> {
    entries.iter().find(|e| e.key == key).map(|e| e.value.clone())
}
