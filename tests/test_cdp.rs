use toy_tcpip::cdp::{CdpNeighborTable, CdpPacket, CDP_MULTICAST_MAC, CDP_SNAP_HEADER, CDP_TLV_DEVICE_ID, CDP_TLV_PORT_ID};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_cdp_packet_encoding_and_tlvs() {
    let pkt = CdpPacket::build("Core-RTR-01", "TenGigabitEthernet0/1", "Cisco ASR-1001", Ipv4Address::new(172, 16, 0, 1));
    let raw = pkt.serialize();

    let parsed = CdpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.ttl, 180);
    assert_eq!(CDP_MULTICAST_MAC.0, [0x01, 0x00, 0x0C, 0xCC, 0xCC, 0xCC]);
    assert_eq!(CDP_SNAP_HEADER.len(), 8);

    let mut dev_id = None;
    let mut port_id = None;
    for tlv in &parsed.tlvs {
        if tlv.tlv_type == CDP_TLV_DEVICE_ID {
            dev_id = Some(String::from_utf8_lossy(&tlv.value).to_string());
        } else if tlv.tlv_type == CDP_TLV_PORT_ID {
            port_id = Some(String::from_utf8_lossy(&tlv.value).to_string());
        }
    }

    assert_eq!(dev_id.as_deref(), Some("Core-RTR-01"));
    assert_eq!(port_id.as_deref(), Some("TenGigabitEthernet0/1"));
}

#[test]
fn test_cdp_neighbor_table_ingest() {
    let pkt = CdpPacket::build("Switch-Edge-24", "FastEthernet0/24", "Cisco Catalyst 2960", Ipv4Address::new(192, 168, 10, 2));
    let mut table = CdpNeighborTable::new();
    table.ingest_packet(&pkt);

    assert_eq!(table.neighbors.len(), 1);
    let n = table.neighbors.get("Switch-Edge-24").unwrap();
    assert_eq!(n.platform, "Cisco Catalyst 2960");
    assert_eq!(n.ip_address, Some(Ipv4Address::new(192, 168, 10, 2)));
}
