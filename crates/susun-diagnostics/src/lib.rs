//! Structured diagnostics and deterministic report collection for `susun`.

pub mod code;
pub mod diagnostic;
pub mod report;

pub use code::{DiagnosticCode, Severity};
pub use diagnostic::{Diagnostic, Label};
pub use report::DiagnosticReport;
