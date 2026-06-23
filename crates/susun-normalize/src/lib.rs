//! Normalize raw parsed Compose input into canonical model types.
//!
//! The `input` module contains the boundary types produced by `susun-loader`.
//! Call [`normalize::normalize`] to convert those types to the canonical model.

pub mod error;
pub mod input;
pub mod normalize;
pub mod provenance;
