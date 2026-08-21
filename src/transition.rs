//! IPv6 Dual-Stack Transition Tunneling (6in4 - RFC 4213 & 4in6 - RFC 2473).
//!
//! Encapsulates IPv6 across legacy IPv4 networks (IP Protocol 41) and IPv4 across IPv6 backbones (Next Header 4).

use crate::ipv4::{Ipv4Address, Ipv4Packet};
use crate::ipv6::{Ipv6Address, Ipv6Packet};
use std::fmt;

pub const IP_PROTO_IPV6_IN_IPV4: u8 = 41; // 6in4 Tunneling
pub const NEXT_HEADER_IPV4_IN_IPV6: u8 = 4; // 4in6 Tunneling

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tunnel6in4 {
    pub local_ipv4: Ipv4Address,
    pub remote_ipv4: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tunnel4in6 {
    pub local_ipv6: Ipv6Address,
    pub remote_ipv6: Ipv6Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    InvalidProtocol(u8),
    PacketTooShort(usize),
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransitionError::InvalidProtocol(p) => write!(f, "Invalid transition tunnel protocol: {}", p),
            TransitionError::PacketTooShort(l) => write!(f, "Transition tunnel packet too short ({} bytes)", l),
        }
    }
}

impl std::error::Error for TransitionError {}

impl Tunnel6in4 {
    pub fn new(local_ipv4: Ipv4Address, remote_ipv4: Ipv4Address) -> Self {
        Tunnel6in4 {
            local_ipv4,
            remote_ipv4,
        }
    }

    pub fn encapsulate(&self, ip6_packet_bytes: &[u8], seq: u16) -> Vec<u8> {
        Ipv4Packet::serialize(
            self.local_ipv4,
            self.remote_ipv4,
            IP_PROTO_IPV6_IN_IPV4,
            seq,
            64,
            ip6_packet_bytes,
        )
    }

    pub fn decapsulate<'a>(&self, ip4_packet: &'a Ipv4Packet) -> Result<&'a [u8], TransitionError> {
        if ip4_packet.header.protocol.to_u8() != IP_PROTO_IPV6_IN_IPV4 {
            return Err(TransitionError::InvalidProtocol(ip4_packet.header.protocol.to_u8()));
        }
        Ok(ip4_packet.payload)
    }
}

impl Tunnel4in6 {
    pub fn new(local_ipv6: Ipv6Address, remote_ipv6: Ipv6Address) -> Self {
        Tunnel4in6 {
            local_ipv6,
            remote_ipv6,
        }
    }

    pub fn encapsulate(&self, ip4_packet_bytes: &[u8]) -> Vec<u8> {
        Ipv6Packet::serialize(
            self.local_ipv6,
            self.remote_ipv6,
            NEXT_HEADER_IPV4_IN_IPV6,
            64,
            ip4_packet_bytes,
        )
    }

    pub fn decapsulate<'a>(&self, ip6_packet: &'a Ipv6Packet) -> Result<&'a [u8], TransitionError> {
        if ip6_packet.header.next_header != NEXT_HEADER_IPV4_IN_IPV6 {
            return Err(TransitionError::InvalidProtocol(ip6_packet.header.next_header));
        }
        Ok(ip6_packet.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_6in4_encapsulation_and_decapsulation() {
        let local_v4 = Ipv4Address::new(198, 51, 100, 1);
        let remote_v4 = Ipv4Address::new(203, 0, 113, 2);
        let tunnel = Tunnel6in4::new(local_v4, remote_v4);

        let inner_ip6 = Ipv6Packet::serialize(
            Ipv6Address::from_str("2001:db8:1::1").unwrap(),
            Ipv6Address::from_str("2001:db8:2::2").unwrap(),
            59, // No Next Header
            64,
            b"6in4 Tunnel Encapsulated IPv6 Traffic",
        );

        let encap = tunnel.encapsulate(&inner_ip6, 101);
        let parsed_ip4 = Ipv4Packet::parse(&encap, true).unwrap();
        assert_eq!(parsed_ip4.header.protocol.to_u8(), IP_PROTO_IPV6_IN_IPV4);
        assert_eq!(parsed_ip4.header.src_ip, local_v4);
        assert_eq!(parsed_ip4.header.dst_ip, remote_v4);

        let decapsulated = tunnel.decapsulate(&parsed_ip4).unwrap();
        let parsed_ip6 = Ipv6Packet::parse(decapsulated).unwrap();
        assert_eq!(parsed_ip6.payload, b"6in4 Tunnel Encapsulated IPv6 Traffic");
    }

    #[test]
    fn test_4in6_encapsulation_and_decapsulation() {
        let local_v6 = Ipv6Address::from_str("2001:db8:aaaa::1").unwrap();
        let remote_v6 = Ipv6Address::from_str("2001:db8:bbbb::2").unwrap();
        let tunnel = Tunnel4in6::new(local_v6, remote_v6);

        let inner_ip4 = Ipv4Packet::serialize(
            Ipv4Address::new(10, 1, 1, 1),
            Ipv4Address::new(10, 2, 2, 2),
            0,
            201,
            64,
            b"4in6 Tunnel Encapsulated IPv4 Traffic",
        );

        let encap = tunnel.encapsulate(&inner_ip4);
        let parsed_ip6 = Ipv6Packet::parse(&encap).unwrap();
        assert_eq!(parsed_ip6.header.next_header, NEXT_HEADER_IPV4_IN_IPV6);

        let decapsulated = tunnel.decapsulate(&parsed_ip6).unwrap();
        let parsed_ip4 = Ipv4Packet::parse(decapsulated, true).unwrap();
        assert_eq!(parsed_ip4.payload, b"4in6 Tunnel Encapsulated IPv4 Traffic");
    }
}
