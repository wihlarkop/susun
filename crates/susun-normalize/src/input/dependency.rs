use indexmap::IndexMap;
use susun_source::Spanned;

/// Raw `depends_on` dependency entry.
#[derive(Debug, Clone, Default)]
pub struct RawDependency {
    /// `condition`.
    pub condition: Option<Spanned<String>>,
    /// `restart`.
    pub restart: Option<Spanned<String>>,
    /// `required`.
    pub required: Option<Spanned<String>>,
}

/// Raw `depends_on` map keyed by service name.
pub type RawDependencies = IndexMap<String, RawDependency>;
