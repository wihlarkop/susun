use susun_source::Spanned;

/// Short-form port entry: `"8080:80"`, `"80"`, `"127.0.0.1:8080:80/tcp"`.
#[derive(Debug, Clone)]
pub struct RawPortShort(pub Spanned<String>);

/// Long-form port entry with explicit fields.
#[derive(Debug, Clone, Default)]
pub struct RawPortLong {
    /// Container-side port number or range.
    pub target: Option<Spanned<String>>,
    /// Host-side published port number or range.
    pub published: Option<Spanned<String>>,
    /// Host IP to bind the port on.
    pub host_ip: Option<Spanned<String>>,
    /// Transport protocol (`tcp` or `udp`).
    pub protocol: Option<Spanned<String>>,
    /// Port publication mode (e.g. `host` or `ingress`).
    pub mode: Option<Spanned<String>>,
}

/// A single port mapping entry in either short or long form.
#[derive(Debug, Clone)]
pub enum RawPortEntry {
    /// Short string form: `"8080:80"`.
    Short(RawPortShort),
    /// Explicit mapping form with individual fields.
    Long(RawPortLong),
}
