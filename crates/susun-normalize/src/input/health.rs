use susun_source::Spanned;

use super::RawStringOrList;

/// Raw supported healthcheck fields.
#[derive(Debug, Clone, Default)]
pub struct RawHealthcheck {
    /// `test`.
    pub test: RawStringOrList,
    /// `interval`.
    pub interval: Option<Spanned<String>>,
    /// `timeout`.
    pub timeout: Option<Spanned<String>>,
    /// `start_period`.
    pub start_period: Option<Spanned<String>>,
    /// `retries`.
    pub retries: Option<Spanned<String>>,
    /// `disable`.
    pub disable: Option<Spanned<String>>,
}
