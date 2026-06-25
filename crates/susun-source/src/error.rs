//! Error types for source operations.

use thiserror::Error;

/// Errors that can occur during source map and span operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceError {
    /// Span start offset is greater than end offset.
    #[error("span start {start} is after end {end}")]
    SpanStartAfterEnd {
        /// Inclusive start offset.
        start: u32,
        /// Exclusive end offset.
        end: u32,
    },
    /// Offset exceeds the length of the source content.
    #[error("offset {offset} is out of source bounds (len {len})")]
    OffsetOutOfBounds {
        /// The requested offset.
        offset: u32,
        /// The length of the source content.
        len: u32,
    },
    /// Offset does not lie on a UTF-8 character boundary.
    #[error("offset {offset} is not on a UTF-8 character boundary")]
    NotUtf8Boundary {
        /// The invalid offset.
        offset: u32,
    },
    /// Source ID has not been registered in this map.
    #[error("unknown source id {0}")]
    UnknownSourceId(u32),
}
