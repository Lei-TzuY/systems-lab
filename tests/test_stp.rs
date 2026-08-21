use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::stp::{BridgeId, StpBpdu, StpBridgeEngine, StpPortRole, StpPortState, STP_PROTOCOL_ID};

#[test]
fn test_stp_bpdu_codec() {
    let root = BridgeId::new(4096, MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]));
    let sender = BridgeId::new(32768, MacAddress([0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]));
    let bpdu = StpBpdu::build_config_bpdu(sender, root, 4, 0x8002);

    let raw = bpdu.serialize();
    let parsed = StpBpdu::parse(&raw).unwrap();

    assert_eq!(parsed.protocol_id, STP_PROTOCOL_ID);
    assert_eq!(parsed.root_id, root);
    assert_eq!(parsed.bridge_id, sender);
    assert_eq!(parsed.root_path_cost, 4);
    assert_eq!(parsed.port_id, 0x8002);
}

#[test]
fn test_stp_bridge_root_election_and_loop_blocking() {
    let mut bridge = StpBridgeEngine::new(32768, MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x03]));
    let superior_root = BridgeId::new(4096, MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x01]));
    let competitor_bridge = BridgeId::new(8192, MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x02]));

    // 1. Ingest superior root on port 1
    let bpdu1 = StpBpdu::build_config_bpdu(superior_root, superior_root, 0, 0x8001);
    bridge.process_bpdu(1, &bpdu1);
    assert_eq!(bridge.root_id, superior_root);
    assert_eq!(bridge.port_states.get(&1), Some(&(StpPortRole::RootPort, StpPortState::Forwarding)));

    // 2. Ingest BPDU from competitor on port 2 with same root but lower bridge ID -> Port 2 is blocked!
    let bpdu2 = StpBpdu::build_config_bpdu(competitor_bridge, superior_root, 19, 0x8002);
    bridge.process_bpdu(2, &bpdu2);
    assert_eq!(bridge.port_states.get(&2), Some(&(StpPortRole::BlockedPort, StpPortState::Blocking)));
}
