//! Pure execution planning for Susun projects.
//!
//! The planner converts Phase 1 analysis outputs plus explicit neutral engine
//! inputs into deterministic, explainable execution plans. It performs no
//! daemon calls and does not mutate the host.

pub mod naming;

pub use naming::{ComposeCompatibleNamingPolicy, NamingError, NamingPolicy, SusunNamingPolicy};
