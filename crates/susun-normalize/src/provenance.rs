//! Source provenance — maps canonical model fields back to their origin spans.

use indexmap::IndexMap;
use susun_source::Span;

/// Tracks the source origin of a canonical service's fields.
pub struct ServiceProvenance {
    /// Span of the `image:` value, if the field was present.
    pub image_span: Option<Span>,
}

/// Tracks the source origin of the top-level canonical project fields.
pub struct ProjectProvenance {
    /// Span of the `name:` value, if the field was present in the file.
    pub name_span: Option<Span>,
    /// Per-service provenance, keyed by service name string.
    pub services: IndexMap<String, ServiceProvenance>,
}
