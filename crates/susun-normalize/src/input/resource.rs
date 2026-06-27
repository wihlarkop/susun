use indexmap::IndexMap;
use susun_source::Spanned;

/// Raw top-level Compose resource definition.
#[derive(Debug, Clone, Default)]
pub struct RawResourceDefinition {
    /// `external`.
    pub external: Option<Spanned<String>>,
    /// `name`.
    pub name: Option<Spanned<String>>,
    /// `file`.
    pub file: Option<Spanned<String>>,
}

/// Raw service network attachment.
#[derive(Debug, Clone, Default)]
pub struct RawNetworkAttachment {
    /// `aliases`.
    pub aliases: Vec<Spanned<String>>,
}

/// Raw service config or secret mount.
#[derive(Debug, Clone)]
pub struct RawResourceMount {
    /// Source resource name.
    pub source: Spanned<String>,
    /// Optional target.
    pub target: Option<Spanned<String>>,
    /// Optional uid.
    pub uid: Option<Spanned<String>>,
    /// Optional gid.
    pub gid: Option<Spanned<String>>,
    /// Optional file mode.
    pub mode: Option<Spanned<String>>,
}

/// Raw top-level resource map.
pub type RawResources = IndexMap<String, Spanned<RawResourceDefinition>>;
/// Raw service network map.
pub type RawServiceNetworks = IndexMap<String, RawNetworkAttachment>;
