use toy_tcpip::fragment::{IpReassemblyBuffer, fragment_payload};
use toy_tcpip::ipv4::{IP_PROTO_ICMP, Ipv4Address, Ipv4Packet};

#[test]
fn test_large_ip_fragmentation_and_reassembly() {
    let src = Ipv4Address::new(192, 168, 1, 100);
    let dst = Ipv4Address::new(192, 168, 1, 10);
    let payload: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();

    // MTU 1400 bytes
    let frags = fragment_payload(src, dst, IP_PROTO_ICMP, 0x1234, 64, 1400, &payload);
    assert!(frags.len() >= 4);

    let mut reassembly = IpReassemblyBuffer::new();
    let mut finished = None;

    // Shuffle fragments into reassembler
    for frag_data in frags.iter().rev() {
        let pkt = Ipv4Packet::parse(frag_data, true).unwrap();
        let res = reassembly.add_fragment(
            pkt.header.src_ip,
            pkt.header.dst_ip,
            pkt.header.protocol.to_u8(),
            pkt.header.identification,
            pkt.header.fragment_offset,
            pkt.header.more_fragments,
            pkt.payload,
        );
        if let Some(assembled) = res {
            finished = Some(assembled);
        }
    }

    assert_eq!(finished, Some(payload));
}
