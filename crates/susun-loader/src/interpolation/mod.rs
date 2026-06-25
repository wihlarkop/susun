//! Single-pass scalar interpolation for Compose files.
//!
//! Entry point: [`interpolate`].

pub mod eval;
pub mod parser;

pub use eval::interpolate;
