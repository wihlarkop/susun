//! Normalize raw parsed Compose input into canonical model types.
//!
//! The `input` module contains the boundary types produced by `susun-loader`.
//! Call [`normalize::normalize`] to convert those types to the canonical model.

pub mod error;
pub mod expand;
pub mod input;
pub mod normalize;
pub mod port;
pub mod provenance;

pub use expand::expand_project;
