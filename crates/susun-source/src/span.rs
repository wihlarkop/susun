//! Byte-offset spans and spanned values.

use crate::{error::SourceError, source::SourceId};

/// A 0-based byte offset into a source's UTF-8 content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextOffset(pub(crate) u32);

impl TextOffset {
    /// Creates a new [`TextOffset`] from a raw byte position.
    pub fn new(offset: u32) -> Self {
        Self(offset)
    }

    /// Returns the raw byte offset value.
    pub fn value(self) -> u32 {
        self.0
    }
}

/// A source span: inclusive start and exclusive end byte offsets within one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// The source this span refers to.
    pub source_id: SourceId,
    /// Inclusive start byte offset.
    pub start: TextOffset,
    /// Exclusive end byte offset.
    pub end: TextOffset,
}

impl Span {
    /// Creates a span, returning an error if `start > end`.
    pub fn new(
        source_id: SourceId,
        start: TextOffset,
        end: TextOffset,
    ) -> Result<Self, SourceError> {
        if start.0 > end.0 {
            return Err(SourceError::SpanStartAfterEnd {
                start: start.0,
                end: end.0,
            });
        }
        Ok(Self {
            source_id,
            start,
            end,
        })
    }

    /// Creates a zero-length span at the given offset.
    pub fn empty(source_id: SourceId, at: TextOffset) -> Self {
        Self {
            source_id,
            start: at,
            end: at,
        }
    }
}

/// A one-based line and byte column position within a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineColumn {
    /// One-based line number.
    pub line: u32,
    /// One-based byte column within the line.
    pub column: u32,
}

/// A value paired with its source location.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    /// The wrapped value.
    pub value: T,
    /// Source location of this value.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Wraps a value with its source span.
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}
