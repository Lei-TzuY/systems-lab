use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::udp::{UdpDatagram, UdpSocketTable};

#[test]
fn test_udp_datagram_checksum_verification() {
    let src_ip = Ipv4Address::new(192, 168, 0, 10);
    let dst_ip = Ipv4Address::new(192, 168, 0, 20);
    let payload = b"DNS lookup or telemetry data";

    let mut raw = UdpDatagram::serialize(src_ip, dst_ip, 5353, 53, payload);

    let parsed = UdpDatagram::parse(src_ip, dst_ip, &raw, true).expect("Valid UDP datagram");
    assert_eq!(parsed.src_port, 5353);
    assert_eq!(parsed.dst_port, 53);
    assert_eq!(parsed.payload, payload);

    // Corrupt payload to trigger checksum mismatch
    let last_idx = raw.len() - 1;
    raw[last_idx] ^= 0xFF;
    assert!(UdpDatagram::parse(src_ip, dst_ip, &raw, true).is_err());
}

#[test]
fn test_udp_socket_dispatch() {
    let mut table = UdpSocketTable::new();

    table.bind(53, |_src_ip, _src_port, payload| {
        let mut resp = Vec::new();
        resp.extend_from_slice(b"DNS Response for: ");
        resp.extend_from_slice(payload);
        Some(resp)
    });

    let resp = table.dispatch(Ipv4Address::new(10, 0, 0, 1), 44321, 53, b"example.com");

    assert_eq!(resp, Some(b"DNS Response for: example.com".to_vec()));
}
