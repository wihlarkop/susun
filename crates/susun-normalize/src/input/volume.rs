use susun_source::Spanned;

/// Short-form volume mount: `"/host/path:/container:ro"` or `"named:/container"`.
#[derive(Debug, Clone)]
pub struct RawVolumeShort(pub Spanned<String>);

/// Long-form volume mount with explicit fields.
#[derive(Debug, Clone, Default)]
pub struct RawVolumeLong {
    /// Mount type: `bind`, `volume`, or `tmpfs`.
    pub volume_type: Option<Spanned<String>>,
    /// Host path or named volume source.
    pub source: Option<Spanned<String>>,
    /// Container mount path.
    pub target: Option<Spanned<String>>,
    /// Whether the mount is read-only (`"true"` / `"false"`).
    pub read_only: Option<Spanned<String>>,
}

/// A single volume mount entry in either short or long form.
#[derive(Debug, Clone)]
pub enum RawVolumeMount {
    /// Short string form: `"/host/path:/container:ro"`.
    Short(RawVolumeShort),
    /// Explicit mapping form with individual fields.
    Long(RawVolumeLong),
}
