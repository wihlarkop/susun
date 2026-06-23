//! Environment variable access for Compose context resolution and interpolation.

pub mod dotenv;
pub mod provider;

pub use dotenv::{DotenvEntry, parse_dotenv};
pub use provider::{EnvironmentProvider, MapEnvironment, ProcessEnvironment};
