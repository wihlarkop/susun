use susun_source::Spanned;

/// The raw value of a `command:` or `entrypoint:` field.
///
/// Absent fields default to [`Null`][RawStringOrList::Null] via [`Default`].
#[derive(Debug, Clone, Default)]
pub enum RawStringOrList {
    /// `null` / `~` — field explicitly set to null, or absent.
    #[default]
    Null,
    /// Single string form: `command: "sh -c 'echo hi'"`.
    String(Spanned<String>),
    /// Sequence form (may be empty): `command: ["sh", "-c", "echo hi"]`.
    List(Vec<Spanned<String>>),
}
