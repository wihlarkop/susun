//! Source map: ID allocation and offset-to-line/column resolution.

use crate::{
    error::SourceError,
    source::{LoadedSource, SourceId},
    span::{LineColumn, TextOffset},
};

/// Allocates [`SourceId`]s and resolves byte offsets to rendered positions.
///
/// The only way to obtain a [`SourceId`] is through [`SourceMap::register`].
#[derive(Debug, Default)]
pub struct SourceMap {
    sources: Vec<LoadedSource>,
    line_starts: Vec<Vec<u32>>,
}

impl SourceMap {
    /// Creates an empty [`SourceMap`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a source and returns its unique [`SourceId`].
    pub fn register(&mut self, source: LoadedSource) -> SourceId {
        let id = SourceId(self.sources.len() as u32);
        let starts = compute_line_starts(&source.contents);
        self.sources.push(source);
        self.line_starts.push(starts);
        id
    }

    /// Returns the [`LoadedSource`] for `id`, or `None` if not registered.
    pub fn get(&self, id: SourceId) -> Option<&LoadedSource> {
        self.sources.get(id.0 as usize)
    }

    /// Resolves a byte offset within a registered source to a one-based [`LineColumn`].
    ///
    /// # Errors
    ///
    /// - [`SourceError::UnknownSourceId`] if `id` is not registered in this map.
    /// - [`SourceError::OffsetOutOfBounds`] if `offset` exceeds the source length.
    /// - [`SourceError::NotUtf8Boundary`] if `offset` falls inside a multi-byte UTF-8 sequence.
    pub fn resolve(&self, id: SourceId, offset: TextOffset) -> Result<LineColumn, SourceError> {
        let source = self.get(id).ok_or(SourceError::UnknownSourceId(id.0))?;
        let contents: &str = &source.contents;
        let offset_usize = offset.0 as usize;

        if offset_usize > contents.len() {
            return Err(SourceError::OffsetOutOfBounds {
                offset: offset.0,
                len: contents.len() as u32,
            });
        }

        if !contents.is_char_boundary(offset_usize) {
            return Err(SourceError::NotUtf8Boundary { offset: offset.0 });
        }

        let starts = &self.line_starts[id.0 as usize];
        let line_idx = starts.partition_point(|&s| s <= offset.0).saturating_sub(1);
        let line_start = starts[line_idx];
        let column = offset.0 - line_start + 1;
        let line = line_idx as u32 + 1;

        Ok(LineColumn { line, column })
    }
}

fn compute_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}
