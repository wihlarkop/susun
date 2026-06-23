//! Raw parsed representation of a Compose file, produced by `susun-loader`.
//!
//! These types form the boundary between the parser adapter (which knows about
//! YAML node types) and the normalizer (which knows about Compose semantics).
//! No `saphyr` or other parser-vendor types appear here.

/// Raw `command` / `entrypoint` types.
pub mod command;
/// Raw `environment` / `labels` types.
pub mod environment;
/// Raw `ports` types.
pub mod port;
/// Raw service entry type.
pub mod service;
/// Raw `volumes` types.
pub mod volume;

pub use command::RawStringOrList;
pub use environment::RawMapping;
pub use port::{RawPortEntry, RawPortLong, RawPortShort};
pub use service::ParsedService;
pub use volume::{RawVolumeMount, RawVolumeLong, RawVolumeShort};

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

/// Merged representation passed to the normalizer.
///
/// For Phase 1, this is a direct 1:1 mapping of a single [`ParsedProject`].
/// Later milestones will expand this to represent the result of merging
/// multiple `-f` files.
pub struct MergeProject {
    /// The top-level `name:` field, if present.
    pub name: Option<Spanned<String>>,
    /// Services, keyed by service name.
    pub services: IndexMap<String, Spanned<ParsedService>>,
}

impl From<ParsedProject> for MergeProject {
    fn from(p: ParsedProject) -> Self {
        MergeProject { name: p.name, services: p.services }
    }
}
