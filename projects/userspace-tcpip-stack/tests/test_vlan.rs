use toy_tcpip::ethernet::{ETHERTYPE_IPV6, EtherType, MacAddress};
use toy_tcpip::vlan::{TaggedEthernetFrame, VlanTag};

#[test]
fn test_vlan_8021q_tagging_and_stripping() {
    let src = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    let dst = MacAddress([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]);
    let vlan = VlanTag::new(4094, 7); // Max VID 4094, Highest PCP 7
    let payload = b"Sensitive VLAN 4094 Isolated Traffic";

    let tagged = TaggedEthernetFrame::serialize(dst, src, vlan, ETHERTYPE_IPV6, payload);
    let parsed = TaggedEthernetFrame::parse(&tagged).unwrap();

    assert_eq!(parsed.vlan.vid, 4094);
    assert_eq!(parsed.vlan.pcp, 7);
    assert_eq!(parsed.inner_ethertype, EtherType::IPv6);
    assert_eq!(parsed.payload, payload);

    let untagged = TaggedEthernetFrame::strip_vlan(&tagged).unwrap();
    assert_eq!(untagged.len(), 14 + payload.len());
}
