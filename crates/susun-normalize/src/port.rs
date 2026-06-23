//! Canonical port mapping types and short-form parser.
//!
//! Handles `"80"`, `"8080:80"`, `"127.0.0.1:8080:80/tcp"`,
//! and range forms `"8080-8090:80"`.

use thiserror::Error;

use crate::input::port::{RawPortEntry, RawPortLong, RawPortShort};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors produced when parsing a port mapping string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortParseError {
    /// The string does not match any recognised port syntax.
    #[error("invalid port format: {0:?}")]
    InvalidFormat(String),
    /// A port number is outside the valid 1–65535 range.
    #[error("port number out of range (must be 1..=65535): {0:?}")]
    OutOfRange(String),
    /// A range has start > end.
    #[error("port range start must not exceed end: {start}-{end}")]
    InvalidRange { start: u16, end: u16 },
    /// The protocol suffix is not recognised.
    #[error("unknown protocol {0:?} (expected tcp, udp, or sctp)")]
    UnknownProtocol(String),
}

// ── Canonical types ───────────────────────────────────────────────────────────

/// Transport protocol for a port mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// Canonical port mapping produced after parsing a raw port entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPort {
    /// Host IP address to bind, if specified.
    pub host_ip: Option<String>,
    /// Host-side published port(s). `None` means no host-port binding (expose only).
    pub published: Option<PublishedPort>,
    /// Container-side target port.
    pub target: u16,
    /// Transport protocol (defaults to TCP).
    pub protocol: Protocol,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Parse a [`RawPortEntry`] into a [`CanonicalPort`].
pub fn parse_port_entry(entry: &RawPortEntry) -> Result<CanonicalPort, PortParseError> {
    match entry {
        RawPortEntry::Short(short) => parse_short(short),
        RawPortEntry::Long(long) => parse_long(long),
    }
}

// ── Short-form parser ─────────────────────────────────────────────────────────

/// Parse a short-form port string such as `"8080:80"` or `"127.0.0.1:8080:80/tcp"`.
pub fn parse_short(short: &RawPortShort) -> Result<CanonicalPort, PortParseError> {
    parse_short_str(short.0.value.as_str())
}

fn parse_short_str(s: &str) -> Result<CanonicalPort, PortParseError> {
    // Split off optional `/protocol` suffix.
    let (body, protocol) = split_protocol(s)?;

    // Split on `:` up to 3 segments: [host_ip,] published, target.
    let parts: Vec<&str> = body.splitn(3, ':').collect();

    match parts.len() {
        1 => {
            // "80" — target only, no host binding.
            let target = parse_port_num(parts[0])?;
            Ok(CanonicalPort { host_ip: None, published: None, target, protocol })
        }
        2 => {
            // "8080:80" or "8080-8090:80" — published:target, no host IP.
            let published = parse_published(parts[0])?;
            let target = parse_port_num(parts[1])?;
            Ok(CanonicalPort { host_ip: None, published: Some(published), target, protocol })
        }
        3 => {
            // "127.0.0.1:8080:80" — host_ip:published:target.
            let host_ip = parts[0].to_owned();
            let published = parse_published(parts[1])?;
            let target = parse_port_num(parts[2])?;
            Ok(CanonicalPort { host_ip: Some(host_ip), published: Some(published), target, protocol })
        }
        _ => Err(PortParseError::InvalidFormat(s.to_owned())),
    }
}

// ── Long-form parser ──────────────────────────────────────────────────────────

/// Parse a long-form port mapping from explicit YAML fields.
pub fn parse_long(long: &RawPortLong) -> Result<CanonicalPort, PortParseError> {
    let target = match &long.target {
        Some(s) => parse_port_num(s.value.as_str())?,
        None => return Err(PortParseError::InvalidFormat("long-form port missing `target`".to_owned())),
    };

    let published = long.published.as_ref().map(|p| parse_published(p.value.as_str())).transpose()?;
    let host_ip = long.host_ip.as_ref().map(|h| h.value.clone());
    let protocol = long.protocol.as_ref().map(|p| parse_protocol(p.value.as_str())).transpose()?.unwrap_or_default();

    Ok(CanonicalPort { host_ip, published, target, protocol })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn split_protocol(s: &str) -> Result<(&str, Protocol), PortParseError> {
    match s.rfind('/') {
        Some(idx) => Ok((&s[..idx], parse_protocol(&s[idx + 1..])?)),
        None => Ok((s, Protocol::Tcp)),
    }
}

fn parse_protocol(s: &str) -> Result<Protocol, PortParseError> {
    match s {
        "tcp" => Ok(Protocol::Tcp),
        "udp" => Ok(Protocol::Udp),
        "sctp" => Ok(Protocol::Sctp),
        other => Err(PortParseError::UnknownProtocol(other.to_owned())),
    }
}

fn parse_published(s: &str) -> Result<PublishedPort, PortParseError> {
    if let Some(dash) = s.find('-') {
        let start = parse_port_num(&s[..dash])?;
        let end = parse_port_num(&s[dash + 1..])?;
        if start > end {
            return Err(PortParseError::InvalidRange { start, end });
        }
        Ok(PublishedPort::Range { start, end })
    } else {
        Ok(PublishedPort::Single(parse_port_num(s)?))
    }
}

fn parse_port_num(s: &str) -> Result<u16, PortParseError> {
    // Port numbers must be 1–65535. We also reject empty strings.
    if s.is_empty() {
        return Err(PortParseError::InvalidFormat(s.to_owned()));
    }
    let n: u32 = s.parse().map_err(|_| PortParseError::OutOfRange(s.to_owned()))?;
    if n == 0 || n > 65535 {
        return Err(PortParseError::OutOfRange(s.to_owned()));
    }
    Ok(n as u16)
}
