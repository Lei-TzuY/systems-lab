use toy_tcpip::vxlan::{VxlanHeader, VxlanPacket, VXLAN_FLAG_VNI_VALID};

#[test]
fn test_vxlan_header_and_24bit_vni() {
    let hdr = VxlanHeader::new(16_000_000).unwrap();
    let raw = hdr.serialize();

    assert_eq!(raw[0], VXLAN_FLAG_VNI_VALID);
    let parsed = VxlanHeader::parse(&raw).unwrap();
    assert_eq!(parsed.vni, 16_000_000);

    // Exceeds 24-bit VNI limit
    assert!(VxlanHeader::new(0x0100_0000).is_err());
}

#[test]
fn test_vxlan_overlay_packet_roundtrip() {
    let original_frame = b"SIMULATED_ETHERNET_PAYLOAD_FOR_VIRTUAL_MACHINE";
    let vxlan_raw = VxlanPacket::encapsulate(42001, original_frame).unwrap();

    let (vni, extracted) = VxlanPacket::decapsulate(&vxlan_raw).unwrap();
    assert_eq!(vni, 42001);
    assert_eq!(extracted, original_frame);
}
