//! YAML parsing adapter — saphyr types stay within this module.
//!
//! The `parse` function accepts an [`EnvResolver`][crate::environment::resolve::EnvResolver]
//! and interpolates scalar values before typed extraction.

mod adapter;

pub(crate) use adapter::parse;
