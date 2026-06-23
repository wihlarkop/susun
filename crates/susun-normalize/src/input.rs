//! Raw parsed representation of a Compose file, produced by `susun-loader`.
//!
//! These types form the boundary between the parser adapter (which knows about
//! YAML node types) and the normalizer (which knows about Compose semantics).
//! No `saphyr` or other parser-vendor types appear here.

use indexmap::IndexMap;
use susun_source::Spanned;

/// Raw parsed representation of a single Compose file.
///
/// All string values carry their source location via [`Spanned`].
/// Fields absent in the file are `None` or empty; the normalizer
/// handles defaults and semantic errors.
pub struct ParsedProject {
    /// The top-level `name:` field, if present.
    pub name: Option<Spanned<String>>,
    /// Services declared under `services:`, keyed by service name.
    pub services: IndexMap<String, Spanned<ParsedService>>,
}

/// Raw parsed representation of a single service entry.
pub struct ParsedService {
    /// The `image:` field, if present.
    pub image: Option<Spanned<String>>,
}
