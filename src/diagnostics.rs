//! Network Diagnostics: Traceroute, ICMP Time Exceeded (Type 11), and Path MTU Discovery (RFC 1191).
//!
//! Generates ICMP error messages (TTL Exceeded in Transit, Frag Needed) and drives hop-by-hop path discovery.

use crate::checksum::compute_checksum;
use crate::ipv4::Ipv4Address;
use std::fmt;

pub const ICMP_TYPE_DEST_UNREACHABLE: u8 = 3;
pub const ICMP_CODE_FRAG_NEEDED: u8 = 4;
pub const ICMP_TYPE_TIME_EXCEEDED: u8 = 11;
pub const ICMP_CODE_TTL_EXPIRED: u8 = 0;

#[derive(Debug, Clone, PartialEq)]
pub struct TracerouteHopResult {
    pub hop: u8,
    pub responder_ip: Option<Ipv4Address>,
    pub rtt_ms: f64,
    pub reached: bool,
}

impl fmt::Display for TracerouteHopResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.responder_ip {
            Some(ip) => write!(f, "{:>2}  {:<16}  {:.2} ms", self.hop, ip, self.rtt_ms),
            None => write!(f, "{:>2}  * * * (Request timed out)", self.hop),
        }
    }
}

/// Builds an ICMP Time Exceeded packet (Type 11, Code 0)
///
/// Encapsulates the 8-byte ICMP header + the original IP header + first 8 bytes of original transport payload.
pub fn build_icmp_time_exceeded(original_ip_packet: &[u8]) -> Vec<u8> {
    let payload_slice_len = original_ip_packet.len().min(28); // 20B IP header + 8B transport
    let mut buf = Vec::with_capacity(8 + payload_slice_len);

    buf.push(ICMP_TYPE_TIME_EXCEEDED);
    buf.push(ICMP_CODE_TTL_EXPIRED);
    buf.extend_from_slice(&[0, 0]); // Checksum placeholder
    buf.extend_from_slice(&[0, 0, 0, 0]); // Unused 4 bytes

    buf.extend_from_slice(&original_ip_packet[..payload_slice_len]);

    let csum = compute_checksum(&buf);
    buf[2] = (csum >> 8) as u8;
    buf[3] = (csum & 0xFF) as u8;

    buf
}

/// Builds an ICMP Destination Unreachable / Fragmentation Needed packet (Type 3, Code 4 - RFC 1191 PMTUD)
///
/// Bytes 6..7 in the ICMP header contain the Next-Hop MTU of the bottleneck link.
pub fn build_icmp_frag_needed(next_hop_mtu: u16, original_ip_packet: &[u8]) -> Vec<u8> {
    let payload_slice_len = original_ip_packet.len().min(28);
    let mut buf = Vec::with_capacity(8 + payload_slice_len);

    buf.push(ICMP_TYPE_DEST_UNREACHABLE);
    buf.push(ICMP_CODE_FRAG_NEEDED);
    buf.extend_from_slice(&[0, 0]); // Checksum placeholder
    buf.extend_from_slice(&[0, 0]); // Unused 2 bytes
    buf.extend_from_slice(&next_hop_mtu.to_be_bytes()); // 2-byte Next-Hop MTU (RFC 1191)

    buf.extend_from_slice(&original_ip_packet[..payload_slice_len]);

    let csum = compute_checksum(&buf);
    buf[2] = (csum >> 8) as u8;
    buf[3] = (csum & 0xFF) as u8;

    buf
}

/// Parses the suggested Next-Hop MTU from an ICMP Frag-Needed packet (RFC 1191)
pub fn parse_pmtud_next_hop_mtu(icmp_data: &[u8]) -> Option<u16> {
    if icmp_data.len() >= 8 && icmp_data[0] == ICMP_TYPE_DEST_UNREACHABLE && icmp_data[1] == ICMP_CODE_FRAG_NEEDED {
        Some(u16::from_be_bytes([icmp_data[6], icmp_data[7]]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_exceeded_generation_and_checksum() {
        let fake_orig_ip = vec![0x45, 0x00, 0x00, 0x3c, 0x01, 0x02, 0x03, 0x04, 0x01, 0x01];
        let icmp = build_icmp_time_exceeded(&fake_orig_ip);

        assert_eq!(icmp[0], ICMP_TYPE_TIME_EXCEEDED);
        assert_eq!(icmp[1], ICMP_CODE_TTL_EXPIRED);
        assert_eq!(compute_checksum(&icmp), 0);
    }

    #[test]
    fn test_pmtud_frag_needed_next_hop_mtu() {
        let fake_orig_ip = vec![0x45; 30];
        let icmp = build_icmp_frag_needed(1400, &fake_orig_ip);

        assert_eq!(icmp[0], ICMP_TYPE_DEST_UNREACHABLE);
        assert_eq!(icmp[1], ICMP_CODE_FRAG_NEEDED);
        assert_eq!(compute_checksum(&icmp), 0);

        let mtu = parse_pmtud_next_hop_mtu(&icmp);
        assert_eq!(mtu, Some(1400));
    }
}
