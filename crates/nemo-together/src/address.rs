//! The validated address of a Listen Together server.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use nemo_core::together::RoomError;

/// The address of a Listen Together server, checked once.
///
/// Accepted forms are `wss://host[:port]/path` and, only for a loopback, private-range or
/// `.local` host, `ws://host[:port]/path`. The host is a name, an IPv4 address or a bracketed
/// IPv6 address. User info, a query and a fragment are refused. There is no default address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerAddress {
    secure: bool,
    host: String,
    port: Option<u16>,
    path: String,
}

impl ServerAddress {
    /// Checks `text` and returns the address.
    ///
    /// The scheme is case-insensitive and the host is lower-cased. An empty path becomes `/`.
    ///
    /// # Errors
    /// [`RoomError::BadAddress`] for any other form, including `ws://` to a public host.
    pub fn parse(text: &str) -> Result<Self, RoomError> {
        let (secure, rest) = split_scheme(text).ok_or(RoomError::BadAddress)?;
        let (authority, path) = match rest.find('/') {
            Some(i) => rest.split_at(i),
            None => (rest, "/"),
        };
        if !valid_path(path) {
            return Err(RoomError::BadAddress);
        }
        let (host, port) = split_host_port(authority).ok_or(RoomError::BadAddress)?;
        let host = host.to_ascii_lowercase();
        if !secure && !is_local_host(&host) {
            return Err(RoomError::BadAddress);
        }
        Ok(Self {
            secure,
            host,
            port,
            path: path.to_owned(),
        })
    }

    /// True for `wss://`.
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.secure
    }

    /// The host, lower-case. An IPv6 address keeps its brackets.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port, when the text had one.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// The path, always starting with `/`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scheme = if self.secure { "wss" } else { "ws" };
        write!(f, "{scheme}://{}", self.host)?;
        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }
        write!(f, "{}", self.path)
    }
}

fn split_scheme(text: &str) -> Option<(bool, &str)> {
    let (scheme, rest) = text.split_once("://")?;
    if scheme.eq_ignore_ascii_case("wss") {
        Some((true, rest))
    } else if scheme.eq_ignore_ascii_case("ws") {
        Some((false, rest))
    } else {
        None
    }
}

/// Printable ASCII only, so no spaces, control characters, query, fragment or backslash.
fn valid_path(path: &str) -> bool {
    path.bytes()
        .all(|b| b.is_ascii_graphic() && !matches!(b, b'?' | b'#' | b'\\'))
}

fn split_host_port(authority: &str) -> Option<(&str, Option<u16>)> {
    let (host, port_text) = if let Some(rest) = authority.strip_prefix('[') {
        let (inner, after) = rest.split_once(']')?;
        inner.parse::<Ipv6Addr>().ok()?;
        let host = &authority[..inner.len() + 2];
        match after {
            "" => (host, None),
            _ => (host, Some(after.strip_prefix(':')?)),
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port_text {
        None => None,
        Some(p) => Some(parse_port(p)?),
    };
    if !host.starts_with('[') && !valid_host(host) {
        return None;
    }
    Some((host, port))
}

fn parse_port(text: &str) -> Option<u16> {
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<u16>().ok().filter(|p| *p != 0)
}

/// A dotted IPv4 address, or a DNS name whose last label is not all digits (so `127.1` and
/// `0x7f.1` are refused instead of being read as an address by someone else).
fn valid_host(host: &str) -> bool {
    if host.parse::<Ipv4Addr>().is_ok() {
        return true;
    }
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let label_ok = |l: &&str| {
        let b = l.as_bytes();
        (1..=63).contains(&b.len())
            && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
            && b[0] != b'-'
            && b[b.len() - 1] != b'-'
    };
    labels.iter().all(label_ok)
        && !labels
            .last()
            .is_some_and(|l| l.bytes().all(|c| c.is_ascii_digit()))
}

/// Loopback, private-range and `.local` hosts, the only ones `ws://` may reach.
fn is_local_host(host: &str) -> bool {
    if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inner
            .parse::<Ipv6Addr>()
            .is_ok_and(|ip| is_local_ip(IpAddr::V6(ip)));
    }
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return is_local_ip(IpAddr::V4(ip));
    }
    host == "localhost"
        || host
            .strip_suffix(".local")
            .is_some_and(|name| !name.is_empty())
}

fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    }
}

#[cfg(test)]
mod tests;
