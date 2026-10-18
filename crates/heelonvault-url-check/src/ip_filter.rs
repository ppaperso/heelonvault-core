//! Classification of destination addresses.
//!
//! Only globally routable unicast addresses are accepted. The check is an
//! allow-list for IPv6 (`2000::/3` minus special-purpose blocks) and a
//! deny-list of every special-purpose IPv4 block registered by IANA, so any
//! unknown or future special range fails closed.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Returns `true` only for addresses considered public Internet destinations.
pub(crate) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}

fn in_v4(ip: Ipv4Addr, net: [u8; 4], prefix: u32) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (u32::from(ip) & mask) == (u32::from(Ipv4Addr::from(net)) & mask)
}

fn in_v6(ip: Ipv6Addr, net: [u16; 8], prefix: u32) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    };
    (u128::from(ip) & mask) == (u128::from(Ipv6Addr::from(net)) & mask)
}

/// IANA IPv4 special-purpose and non-public blocks.
const NON_PUBLIC_V4: &[([u8; 4], u32)] = &[
    ([0, 0, 0, 0], 8),       // "this network", includes unspecified
    ([10, 0, 0, 0], 8),      // private
    ([100, 64, 0, 0], 10),   // shared address space (CGNAT)
    ([127, 0, 0, 0], 8),     // loopback
    ([169, 254, 0, 0], 16),  // link-local
    ([172, 16, 0, 0], 12),   // private
    ([192, 0, 0, 0], 24),    // IETF protocol assignments
    ([192, 0, 2, 0], 24),    // documentation TEST-NET-1
    ([192, 31, 196, 0], 24), // AS112-v4
    ([192, 52, 193, 0], 24), // AMT
    ([192, 88, 99, 0], 24),  // deprecated 6to4 relay anycast
    ([192, 168, 0, 0], 16),  // private
    ([192, 175, 48, 0], 24), // direct delegation AS112
    ([198, 18, 0, 0], 15),   // benchmarking
    ([198, 51, 100, 0], 24), // documentation TEST-NET-2
    ([203, 0, 113, 0], 24),  // documentation TEST-NET-3
    ([224, 0, 0, 0], 4),     // multicast
    ([240, 0, 0, 0], 4),     // reserved, includes limited broadcast
];

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    !NON_PUBLIC_V4
        .iter()
        .any(|&(net, prefix)| in_v4(ip, net, prefix))
}

/// Special-purpose blocks carved out of the global unicast range `2000::/3`.
const NON_PUBLIC_V6_IN_GLOBAL: &[([u16; 8], u32)] = &[
    ([0x2001, 0, 0, 0, 0, 0, 0, 0], 23), // IETF protocol assignments (Teredo, ORCHID, benchmarking...)
    ([0x2001, 0x0db8, 0, 0, 0, 0, 0, 0], 32), // documentation
    ([0x2002, 0, 0, 0, 0, 0, 0, 0], 16), // 6to4 (embeds arbitrary IPv4)
    ([0x3fff, 0, 0, 0, 0, 0, 0, 0], 20), // documentation (RFC 9637)
];

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    // IPv4-mapped (::ffff:a.b.c.d): judged by the embedded IPv4 address.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(v4);
    }
    // NAT64 well-known prefix (64:ff9b::/96): judged by the embedded IPv4 address.
    if in_v6(ip, [0x0064, 0xff9b, 0, 0, 0, 0, 0, 0], 96) {
        let [.., a, b, c, d] = ip.octets();
        return is_public_ipv4(Ipv4Addr::new(a, b, c, d));
    }
    // Everything else (unspecified, loopback, IPv4-compatible, local-use NAT64,
    // discard, ULA, link-local, site-local, multicast...) lies outside 2000::/3.
    if !in_v6(ip, [0x2000, 0, 0, 0, 0, 0, 0, 0], 3) {
        return false;
    }
    !NON_PUBLIC_V6_IN_GLOBAL
        .iter()
        .any(|&(net, prefix)| in_v6(ip, net, prefix))
}
