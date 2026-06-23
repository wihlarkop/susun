//! Canonical port-mapping model types.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Transport protocol for a port mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Protocol {
    /// TCP (default when no protocol is specified).
    #[default]
    Tcp,
    /// UDP.
    Udp,
    /// SCTP.
    Sctp,
}

/// Published (host-side) port number or range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum PublishedPort {
    /// A single host port.
    Single(u16),
    /// A contiguous range of host ports `[start, end]` (inclusive).
    Range {
        /// First port in the range.
        start: u16,
        /// Last port in the range (inclusive).
        end: u16,
    },
}

/// Canonical port mapping in the project model.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CanonicalPort {
    /// Host IP address to bind, if specified.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub host_ip: Option<String>,
    /// Host-side published port(s). `None` means no host-port binding (expose only).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub published: Option<PublishedPort>,
    /// Container-side target port.
    pub target: u16,
    /// Transport protocol.
    pub protocol: Protocol,
}
