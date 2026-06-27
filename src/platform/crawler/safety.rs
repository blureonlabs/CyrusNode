//! SSRF guard. Resolves the host of every URL we're about to fetch and rejects
//! any that resolve to loopback, private (RFC1918), link-local, ULA, multicast,
//! cloud-metadata, or unspecified addresses.
//!
//! Called by the three places in this crate that do outbound HTTP:
//! [`super::client::HttpCrawler::crawl`], [`super::robots::RobotsCache::fetch`],
//! and [`super::sitemap::try_fetch_sitemap`]. Without this guard, an operator
//! (or, in V2, an attacker) could point the crawler at `169.254.169.254` and
//! exfiltrate cloud instance credentials, or scan internal services via
//! `http://10.0.0.5/`.
//!
//! Defense in depth, not a substitute for boundary controls: the redirect
//! policy in `HttpCrawler::new` is also pinned to `Policy::none()` so a
//! 30x to a private IP cannot bypass this guard.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum SafetyError {
    #[error("url has no host")]
    NoHost,
    #[error("dns lookup failed for {host}: {source}")]
    DnsLookup {
        host: String,
        #[source]
        source: std::io::Error,
    },
    #[error("dns returned no addresses for {0}")]
    NoAddresses(String),
    #[error("blocked: {host} resolves to non-public address {addr}")]
    BlockedAddress { host: String, addr: IpAddr },
}

/// Returns `Ok(())` only when every IP the host resolves to is publicly routable.
///
/// Conservative on purpose: a single private resolution rejects the URL, even
/// if other resolutions are public. This stops DNS rebinding-style smuggling
/// where one of several A records is private.
pub async fn ensure_public_host(url: &Url) -> Result<(), SafetyError> {
    let host = url.host_str().ok_or(SafetyError::NoHost)?.to_string();
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| SafetyError::DnsLookup {
            host: host.clone(),
            source: e,
        })?;

    let mut any = false;
    let mut blocked: Option<IpAddr> = None;
    for sa in addrs {
        any = true;
        let ip = sa.ip();
        if !is_public_ip(ip) {
            blocked = Some(ip);
            break;
        }
    }
    if let Some(addr) = blocked {
        return Err(SafetyError::BlockedAddress { host, addr });
    }
    if !any {
        return Err(SafetyError::NoAddresses(host));
    }
    Ok(())
}

/// `true` only if the address is safe to dial: not loopback, private,
/// link-local, ULA, multicast, broadcast, or unspecified.
fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(v4: Ipv4Addr) -> bool {
    if v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_multicast()
        || v4.is_broadcast()
        || v4.is_unspecified()
        || v4.is_documentation()
    {
        return false;
    }
    // 100.64.0.0/10 — Carrier-grade NAT
    let oct = v4.octets();
    if oct[0] == 100 && (oct[1] & 0xc0) == 64 {
        return false;
    }
    // 192.0.0.0/24 (IETF assignments) and 192.0.2.0/24 (TEST-NET-1) covered by is_documentation.
    // 198.18.0.0/15 — benchmarking
    if oct[0] == 198 && (oct[1] == 18 || oct[1] == 19) {
        return false;
    }
    true
}

fn is_public_v6(v6: Ipv6Addr) -> bool {
    if v6.is_loopback() || v6.is_multicast() || v6.is_unspecified() {
        return false;
    }
    let segs = v6.segments();
    // link-local fe80::/10
    if (segs[0] & 0xffc0) == 0xfe80 {
        return false;
    }
    // ULA fc00::/7
    let first = v6.octets()[0];
    if (first & 0xfe) == 0xfc {
        return false;
    }
    // ::ffff:0:0/96 (IPv4-mapped) — reject; the v4 ought to be validated as v4 by lookup_host.
    if segs[0] == 0
        && segs[1] == 0
        && segs[2] == 0
        && segs[3] == 0
        && segs[4] == 0
        && segs[5] == 0xffff
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_v4_private_ranges() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254", // AWS / GCP metadata
            "100.64.0.1",
            "198.18.0.1",
            "0.0.0.0",
        ] {
            let ip: Ipv4Addr = ip.parse().expect("valid v4");
            assert!(!is_public_v4(ip), "expected {ip} to be non-public");
        }
    }

    #[test]
    fn allows_v4_public_ranges() {
        for ip in ["8.8.8.8", "1.1.1.1", "13.32.5.10", "104.21.0.1"] {
            let ip: Ipv4Addr = ip.parse().expect("valid v4");
            assert!(is_public_v4(ip), "expected {ip} to be public");
        }
    }

    #[test]
    fn blocks_v6_ula_and_link_local() {
        for ip in ["::1", "fe80::1", "fc00::1", "fd00::1", "::"] {
            let ip: Ipv6Addr = ip.parse().expect("valid v6");
            assert!(!is_public_v6(ip), "expected {ip} to be non-public");
        }
    }

    #[test]
    fn allows_v6_public() {
        for ip in ["2606:4700:4700::1111", "2001:4860:4860::8888"] {
            let ip: Ipv6Addr = ip.parse().expect("valid v6");
            assert!(is_public_v6(ip), "expected {ip} to be public");
        }
    }
}
