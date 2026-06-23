use indexmap::IndexMap;
use susun_source::Spanned;

/// Raw environment or label entries in either mapping or sequence form.
///
/// Absent fields default to [`Map`][RawMapping::Map] with an empty map via [`Default`].
#[derive(Debug, Clone)]
pub enum RawMapping {
    /// `key: value` mapping form. Value is `None` when the YAML value is null.
    Map(IndexMap<String, Option<Spanned<String>>>),
    /// `- KEY=value` or `- KEY` sequence form.
    List(Vec<Spanned<String>>),
}

impl Default for RawMapping {
    fn default() -> Self {
        RawMapping::Map(IndexMap::new())
    }
}
