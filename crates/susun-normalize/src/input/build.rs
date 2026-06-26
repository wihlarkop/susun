use indexmap::IndexMap;
use susun_source::Spanned;

/// Raw parsed service build definition.
#[derive(Debug, Clone, Default)]
pub struct RawBuildDefinition {
    /// Build context.
    pub context: Option<Spanned<String>>,
    /// Dockerfile path.
    pub dockerfile: Option<Spanned<String>>,
    /// Target stage.
    pub target: Option<Spanned<String>>,
    /// Build arguments.
    pub args: IndexMap<String, Option<Spanned<String>>>,
    /// Target platforms.
    pub platforms: Vec<Spanned<String>>,
    /// Build secrets.
    pub secrets: Vec<Spanned<String>>,
    /// SSH identities.
    pub ssh: Vec<Spanned<String>>,
    /// Cache sources.
    pub cache_from: Vec<Spanned<String>>,
    /// Cache destinations.
    pub cache_to: Vec<Spanned<String>>,
}
