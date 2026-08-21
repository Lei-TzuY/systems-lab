use std::str::FromStr;
use toy_tcpip::bfd::{BfdControlPacket, BfdState};
use toy_tcpip::bfd_v6::{BfdV6Manager, BfdV6Session, BFD_MULTIHOP_PORT};
use toy_tcpip::ipv6::Ipv6Address;

#[test]
fn test_bfd_v6_multi_hop_session_lifecycle() {
    let mut mgr = BfdV6Manager::new();
    let peer_ip = Ipv6Address::from_str("2001:db8:acad::2").unwrap();

    let session = BfdV6Session::new(peer_ip, 0xABCDEF01, true);
    assert_eq!(session.state, BfdState::Down);
    assert_eq!(session.is_multihop, true);

    mgr.add_session(session);

    let sess = mgr.sessions.get_mut(&peer_ip).unwrap();

    // Step 1: Transmit initial Down packet with Poll bit
    let initial_pkt = sess.build_outbound_packet(true);
    assert_eq!(initial_pkt.state, BfdState::Down);
    assert_eq!(initial_pkt.poll, true);
    assert_eq!(initial_pkt.my_discriminator, 0xABCDEF01);

    // Step 2: Receive peer Down packet -> Transition to Init
    let peer_down = BfdControlPacket::build_control(BfdState::Down, 0x99881122, 0, 50_000);
    let init_resp = sess.process_inbound_packet(&peer_down).unwrap();
    assert_eq!(sess.state, BfdState::Init);
    assert_eq!(init_resp.state, BfdState::Init);
    assert_eq!(sess.your_discriminator, 0x99881122);

    // Step 3: Receive peer Init packet -> Transition to Up
    let peer_init = BfdControlPacket::build_control(BfdState::Init, 0x99881122, 0xABCDEF01, 50_000);
    let up_resp = sess.process_inbound_packet(&peer_init).unwrap();
    assert_eq!(sess.state, BfdState::Up);
    assert_eq!(up_resp.state, BfdState::Up);
}

#[test]
fn test_bfd_multihop_port_constant() {
    assert_eq!(BFD_MULTIHOP_PORT, 4784);
}
