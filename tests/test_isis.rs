use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::isis::{IsisHelloPacket, ETHERTYPE_ISIS, ISIS_NLPID_DISCRIMINATOR, ISIS_PDU_L1_LAN_IIH, ISIS_TLV_AREA_ADDRESSES, ISIS_TLV_PROTOCOLS_SUPPORTED};

#[test]
fn test_isis_lan_hello_framing() {
    let sys_id = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
    let area = &[0x49, 0x00, 0x02];
    let ip = Ipv4Address::new(10, 0, 0, 1);

    let hello = IsisHelloPacket::build_l1_lan_hello(sys_id, area, ip);
    let raw = hello.serialize();

    let parsed = IsisHelloPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.nlpid, ISIS_NLPID_DISCRIMINATOR);
    assert_eq!(parsed.header.pdu_type, ISIS_PDU_L1_LAN_IIH);
    assert_eq!(parsed.source_id, sys_id);
    assert_eq!(parsed.priority, 64);
    assert_eq!(parsed.tlvs.len(), 3);
    assert_eq!(parsed.tlvs[0].tlv_type, ISIS_TLV_AREA_ADDRESSES);
    assert_eq!(parsed.tlvs[1].tlv_type, ISIS_TLV_PROTOCOLS_SUPPORTED);
    assert_eq!(ETHERTYPE_ISIS, 0x8870);
}
