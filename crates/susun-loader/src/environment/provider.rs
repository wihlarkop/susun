//! Environment variable providers for Compose context resolution.

use std::collections::BTreeMap;

/// Reads environment variables for Compose interpolation and context resolution.
///
/// Implementations must be deterministic: the same key always returns the same
/// value within a single provider instance. Use [`MapEnvironment`] for tests.
pub trait EnvironmentProvider: Send + Sync {
    /// Returns the value of the named variable, or `None` if not set.
    fn get(&self, key: &str) -> Option<String>;

    /// Returns all variables as sorted `(key, value)` pairs.
    fn vars(&self) -> Vec<(String, String)>;
}

/// Reads variables from the real process environment.
pub struct ProcessEnvironment;

impl EnvironmentProvider for ProcessEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn vars(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = std::env::vars().collect();
        pairs.sort();
        pairs
    }
}

/// A deterministic in-memory environment backed by a sorted map.
///
/// Designed for testing: variable lookup order and `vars()` output are
/// both alphabetically sorted and fully reproducible.
#[derive(Default)]
pub struct MapEnvironment {
    vars: BTreeMap<String, String>,
}

impl MapEnvironment {
    /// Creates a new `MapEnvironment` from a `BTreeMap`.
    pub fn new(vars: BTreeMap<String, String>) -> Self {
        Self { vars }
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for MapEnvironment
where
    K: Into<String>,
    V: Into<String>,
{
    fn from(pairs: [(K, V); N]) -> Self {
        Self {
            vars: pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

impl EnvironmentProvider for MapEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }

    fn vars(&self) -> Vec<(String, String)> {
        self.vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
