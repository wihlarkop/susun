//! Source identity, span, and source map types for `susun`.
//!
//! Provides the low-level primitives that let every other susun crate
//! point back into original source text without depending on any YAML
//! parser types.

pub mod error;
pub mod map;
pub mod source;
pub mod span;

pub use error::SourceError;
pub use map::SourceMap;
pub use source::{LoadedSource, SourceId, SourceName};
pub use span::{LineColumn, Span, Spanned, TextOffset};
