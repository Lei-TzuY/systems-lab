use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::turn::{TurnAllocationTable, TurnPacket, TURN_ALLOCATE_REQUEST, TURN_ALLOCATE_RESPONSE, TURN_DATA_INDICATION, TURN_SEND_INDICATION};

#[test]
fn test_turn_allocation_lifecycle() {
    let tid = [0x99; 12];
    let req = TurnPacket::build_allocate_request(tid, 300);
    let raw_req = req.serialize();

    let parsed_req = TurnPacket::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.msg_type, TURN_ALLOCATE_REQUEST);

    let client_ip = Ipv4Address::new(192, 168, 1, 100);
    let client_port = 50000;
    let rel_ip = Ipv4Address::new(203, 0, 113, 10);
    let rel_port = 49152;

    let mut table = TurnAllocationTable::new();
    let alloc = table.create_allocation(client_ip, client_port, rel_ip, rel_port, 300);
    assert_eq!(alloc.relayed_ip, rel_ip);
    assert_eq!(alloc.relayed_port, rel_port);

    let resp = TurnPacket::build_allocate_response(&parsed_req, rel_ip, rel_port, 300);
    let raw_resp = resp.serialize();

    let parsed_resp = TurnPacket::parse(&raw_resp).unwrap();
    assert_eq!(parsed_resp.msg_type, TURN_ALLOCATE_RESPONSE);
    assert_eq!(parsed_resp.get_xor_relayed_address(), Some((rel_ip, rel_port)));
}

#[test]
fn test_turn_send_and_data_relaying() {
    let peer_ip = Ipv4Address::new(198, 51, 100, 5);
    let peer_port = 6000;
    let msg = b"Encapsulated Relay Data";

    let send = TurnPacket::build_send_indication(peer_ip, peer_port, msg);
    let raw_send = send.serialize();
    let parsed_send = TurnPacket::parse(&raw_send).unwrap();
    assert_eq!(parsed_send.msg_type, TURN_SEND_INDICATION);
    assert_eq!(parsed_send.get_xor_peer_address(), Some((peer_ip, peer_port)));
    assert_eq!(parsed_send.get_data_payload(), Some(msg.as_ref()));

    let data = TurnPacket::build_data_indication(peer_ip, peer_port, msg);
    let raw_data = data.serialize();
    let parsed_data = TurnPacket::parse(&raw_data).unwrap();
    assert_eq!(parsed_data.msg_type, TURN_DATA_INDICATION);
    assert_eq!(parsed_data.get_xor_peer_address(), Some((peer_ip, peer_port)));
}
