//! Canonical domain model for `susun`.
//!
//! This crate has no Susun workspace dependencies. Enable the `serde` feature
//! for JSON serialization of the canonical types.

pub mod name;
pub mod project;

pub use name::{ImageRef, ProjectName, ServiceName};
pub use project::{Project, Service};
