//! Environment variable access for Compose context resolution and interpolation.

pub mod provider;

pub use provider::{EnvironmentProvider, MapEnvironment, ProcessEnvironment};
