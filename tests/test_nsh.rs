use toy_tcpip::nsh::{NshPacket, ServiceFunctionForwarder, NSH_MD_TYPE_1, NSH_NP_ETHERNET, NSH_NP_IPV4};

#[test]
fn test_nsh_base_and_context_headers() {
    let payload = b"Classified Tenant Packet for Service Chaining";
    let spi = 100;
    let si = 255;
    let tenant_id = 4001;
    let flow_hash = 0xDEADBEEF;

    let pkt = NshPacket::build_ipv4(spi, si, tenant_id, flow_hash, payload);
    let raw = pkt.serialize();

    let parsed = NshPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.md_type, NSH_MD_TYPE_1);
    assert_eq!(parsed.header.next_protocol, NSH_NP_IPV4);
    assert_eq!(parsed.header.service_path_id, 100);
    assert_eq!(parsed.header.service_index, 255);
    assert_eq!(parsed.header.context_c2, 4001);
    assert_eq!(parsed.header.context_c4, 0xDEADBEEF);
    assert_eq!(parsed.payload, payload);
}

#[test]
fn test_nsh_service_function_chaining_hops() {
    let mut pkt = NshPacket::build_ethernet(200, 3, 500, 0x11223344, b"Layer 2 Frame");
    assert_eq!(pkt.header.next_protocol, NSH_NP_ETHERNET);

    // Hop 1: Firewall
    assert!(ServiceFunctionForwarder::forward_next_service_hop(&mut pkt));
    assert_eq!(pkt.header.service_index, 2);

    // Hop 2: DPI
    assert!(ServiceFunctionForwarder::forward_next_service_hop(&mut pkt));
    assert_eq!(pkt.header.service_index, 1);

    // Hop 3: WAF / Terminal
    assert!(ServiceFunctionForwarder::forward_next_service_hop(&mut pkt));
    assert_eq!(pkt.header.service_index, 0);

    // End of Chain
    assert_eq!(ServiceFunctionForwarder::forward_next_service_hop(&mut pkt), false);
}
