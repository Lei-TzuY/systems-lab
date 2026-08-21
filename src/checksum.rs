//! RFC 1071 Internet Checksum implementation.
//!
//! The checksum algorithm calculates the 16-bit one's complement sum
//! of all 16-bit words in the packet header or payload. It is used across
//! IPv4 headers, ICMP messages, UDP datagrams, and TCP segments.

/// Computes the raw 16-bit one's complement sum of a byte buffer.
/// If the buffer has an odd length, the last byte is treated as the upper 8 bits
/// of a 16-bit word with 0 as the lower 8 bits (network byte order).
pub fn raw_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);

    for chunk in &mut chunks {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        sum = sum.wrapping_add(word);
    }

    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        // Odd byte padded with trailing zero as lower byte
        let word = ((remainder[0] as u16) << 8) as u32;
        sum = sum.wrapping_add(word);
    }

    sum
}

/// Folds a 32-bit accumulated sum down to a 16-bit one's complement value
/// and returns the bitwise inversion (RFC 1071).
pub fn finalize_checksum(mut sum: u32) -> u16 {
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Computes the RFC 1071 checksum over a single buffer.
pub fn compute_checksum(data: &[u8]) -> u16 {
    let sum = raw_checksum(data);
    finalize_checksum(sum)
}

/// Validates whether a buffer containing its own checksum field evaluates to valid (0x0000 when inverted).
pub fn verify_checksum(data: &[u8]) -> bool {
    let mut sum = raw_checksum(data);
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    sum == 0xFFFF || sum == 0x0000
}

/// Computes a transport-layer checksum (TCP / UDP) using the IPv4 pseudo-header.
pub fn compute_ipv4_transport_checksum(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    protocol: u8,
    transport_segment: &[u8],
) -> u16 {
    let mut sum: u32 = 0;

    // Source IP
    sum = sum.wrapping_add(u16::from_be_bytes([src_ip[0], src_ip[1]]) as u32);
    sum = sum.wrapping_add(u16::from_be_bytes([src_ip[2], src_ip[3]]) as u32);

    // Destination IP
    sum = sum.wrapping_add(u16::from_be_bytes([dst_ip[0], dst_ip[1]]) as u32);
    sum = sum.wrapping_add(u16::from_be_bytes([dst_ip[2], dst_ip[3]]) as u32);

    // Zero + Protocol
    sum = sum.wrapping_add(protocol as u32);

    // Transport Segment Length
    let transport_len = transport_segment.len() as u16;
    sum = sum.wrapping_add(transport_len as u32);

    // Transport Segment data
    sum = sum.wrapping_add(raw_checksum(transport_segment));

    let result = finalize_checksum(sum);
    // For UDP, if computed checksum is 0, RFC 768 specifies transmitting 0xFFFF (since 0 means no checksum)
    if protocol == 17 && result == 0 {
        0xFFFF
    } else {
        result
    }
}

/// Verifies a transport-layer checksum (TCP / UDP) over an incoming segment that already includes its checksum.
pub fn verify_ipv4_transport_checksum(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    protocol: u8,
    transport_segment: &[u8],
) -> bool {
    let mut sum: u32 = 0;

    // Source IP
    sum = sum.wrapping_add(u16::from_be_bytes([src_ip[0], src_ip[1]]) as u32);
    sum = sum.wrapping_add(u16::from_be_bytes([src_ip[2], src_ip[3]]) as u32);

    // Destination IP
    sum = sum.wrapping_add(u16::from_be_bytes([dst_ip[0], dst_ip[1]]) as u32);
    sum = sum.wrapping_add(u16::from_be_bytes([dst_ip[2], dst_ip[3]]) as u32);

    // Zero + Protocol
    sum = sum.wrapping_add(protocol as u32);

    // Transport Segment Length
    let transport_len = transport_segment.len() as u16;
    sum = sum.wrapping_add(transport_len as u32);

    // Transport Segment data
    sum = sum.wrapping_add(raw_checksum(transport_segment));

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    sum == 0xFFFF || sum == 0x0000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc1071_example() {
        // Standard test vector: 0x0001, 0xf203, 0xf4f5, 0xf6f7
        let data = [0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7];
        let csum = compute_checksum(&data);
        assert_eq!(csum, 0x220d);

        // Append checksum and verify
        let mut full = data.to_vec();
        full.extend_from_slice(&csum.to_be_bytes());
        assert!(verify_checksum(&full));
    }

    #[test]
    fn test_odd_length_checksum() {
        let data = [0x45, 0x00, 0x00];
        let csum = compute_checksum(&data);
        assert_ne!(csum, 0);
    }
}
