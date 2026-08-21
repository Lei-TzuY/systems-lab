use toy_tcpip::diagnostics::{
    build_icmp_frag_needed, build_icmp_time_exceeded, parse_pmtud_next_hop_mtu,
    TracerouteHopResult, ICMP_CODE_FRAG_NEEDED, ICMP_CODE_TTL_EXPIRED,
    ICMP_TYPE_DEST_UNREACHABLE, ICMP_TYPE_TIME_EXCEEDED,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_icmp_time_exceeded_and_pmtud_frag_needed() {
    let raw_ip = vec![0x45, 0x00, 0x00, 0x54, 0x11, 0x22, 0x40, 0x00, 0x01, 0x01]; // TTL = 1

    // 1. Time Exceeded
    let time_exceeded = build_icmp_time_exceeded(&raw_ip);
    assert_eq!(time_exceeded[0], ICMP_TYPE_TIME_EXCEEDED);
    assert_eq!(time_exceeded[1], ICMP_CODE_TTL_EXPIRED);
    assert_eq!(toy_tcpip::checksum::compute_checksum(&time_exceeded), 0);

    // 2. Frag Needed with Next-Hop MTU = 1380
    let frag_needed = build_icmp_frag_needed(1380, &raw_ip);
    assert_eq!(frag_needed[0], ICMP_TYPE_DEST_UNREACHABLE);
    assert_eq!(frag_needed[1], ICMP_CODE_FRAG_NEEDED);
    assert_eq!(toy_tcpip::checksum::compute_checksum(&frag_needed), 0);
    assert_eq!(parse_pmtud_next_hop_mtu(&frag_needed), Some(1380));
}

#[test]
fn test_traceroute_hop_formatting() {
    let hop = TracerouteHopResult {
        hop: 1,
        responder_ip: Some(Ipv4Address::new(192, 168, 1, 1)),
        rtt_ms: 0.85,
        reached: false,
    };
    let formatted = format!("{}", hop);
    assert!(formatted.contains("192.168.1.1"));
    assert!(formatted.contains("0.85 ms"));
}
