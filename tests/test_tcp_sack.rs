use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::tcp::*;

#[test]
fn test_tcp_sack_option_codec() {
    let src = Ipv4Address::new(10, 0, 0, 1);
    let dst = Ipv4Address::new(10, 0, 0, 2);

    let sack_blocks = vec![(1000, 2000), (3000, 4000), (5000, 6000)];
    let options = vec![
        TcpOption::SackPermitted,
        TcpOption::Sack(sack_blocks.clone()),
        TcpOption::Timestamp {
            val: 0x11223344,
            ecr: 0x55667788,
        },
    ];

    let serialized = TcpSegment::serialize_with_options(
        src,
        dst,
        1234,
        80,
        100,
        200,
        TcpFlags::ack(),
        65535,
        &options,
        b"data",
    );

    let parsed = TcpSegment::parse(src, dst, &serialized, true).unwrap();
    let non_nop_options: Vec<_> = parsed
        .options
        .iter()
        .cloned()
        .filter(|o| !matches!(o, TcpOption::Nop | TcpOption::EndOfOptions))
        .collect();
    assert_eq!(non_nop_options.len(), 3);
    assert_eq!(non_nop_options[0], TcpOption::SackPermitted);
    assert_eq!(non_nop_options[1], TcpOption::Sack(sack_blocks));
    assert_eq!(
        non_nop_options[2],
        TcpOption::Timestamp {
            val: 0x11223344,
            ecr: 0x55667788
        }
    );
    assert_eq!(parsed.payload, b"data");
}

#[test]
fn test_tcp_sack_negotiation_and_generation() {
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(192, 168, 1, 1),
        port: 10000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(192, 168, 1, 2),
        port: 80,
    };

    let mut server = TcpConnection::new_server(local, remote, 1000);

    // 1. Client sends SYN with SackPermitted
    let syn_options = vec![TcpOption::Mss(1460), TcpOption::SackPermitted];
    let client_syn = TcpSegment::serialize_with_options(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        5000,
        0,
        TcpFlags::syn(),
        65535,
        &syn_options,
        &[],
    );
    let parsed_syn = TcpSegment::parse(remote.ip, local.ip, &client_syn, true).unwrap();
    let syn_ack_raw = server.handle_segment_at(&parsed_syn, 100).unwrap();

    let parsed_syn_ack = TcpSegment::parse(local.ip, remote.ip, &syn_ack_raw, true).unwrap();
    assert!(
        parsed_syn_ack
            .options
            .iter()
            .any(|o| matches!(o, TcpOption::SackPermitted))
    );
    assert!(server.sack_permitted);

    // 2. Client completes handshake with ACK
    let client_ack = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        5001,
        1001,
        TcpFlags::ack(),
        65535,
        &[],
    );
    let parsed_client_ack = TcpSegment::parse(remote.ip, local.ip, &client_ack, true).unwrap();
    server.handle_segment_at(&parsed_client_ack, 110);
    assert_eq!(server.state, TcpState::Established);

    // 3. Receive out-of-order segment [5501..6001], hole is [5001..5501]
    let ooo_payload = vec![0xaa; 500];
    let ooo_seg = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        5501,
        1001,
        TcpFlags::ack(),
        65535,
        &ooo_payload,
    );
    let parsed_ooo = TcpSegment::parse(remote.ip, local.ip, &ooo_seg, true).unwrap();
    let sack_ack_raw = server.handle_segment_at(&parsed_ooo, 120).unwrap();

    let parsed_sack_ack = TcpSegment::parse(local.ip, remote.ip, &sack_ack_raw, true).unwrap();
    assert_eq!(parsed_sack_ack.ack_num, 5001); // Cumulative ACK still at hole start
    let sack_opt = parsed_sack_ack
        .options
        .iter()
        .find(|o| matches!(o, TcpOption::Sack(_)));
    assert!(sack_opt.is_some());
    if let Some(TcpOption::Sack(blocks)) = sack_opt {
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], (5501, 6001));
    }
}

#[test]
fn test_tcp_sack_selective_retransmission_and_timestamps() {
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 5000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };

    let mut client = TcpConnection::new_client(local, remote, 100);
    client.sack_permitted = true;
    client.peer_sack_permitted = true;
    client.state = TcpState::Established;
    client.rcv_nxt = 500;
    client.snd_nxt = 100;
    client.snd_una = 100;

    // Send 3 segments: [100..200], [200..300], [300..400]
    client.send_data_at(&[1u8; 100], 1000);
    client.send_data_at(&[2u8; 100], 1000);
    client.send_data_at(&[3u8; 100], 1000);

    assert_eq!(client.retransmit_queue.len(), 3);

    // Peer ACKs 100 (cumulative), but SACKs [200..400] (segments 2 and 3 arrived, segment 1 lost)
    let sack_opt = vec![TcpOption::Sack(vec![(200, 400)])];
    let ack_seg = TcpSegment::serialize_with_options(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        500,
        100,
        TcpFlags::ack(),
        65535,
        &sack_opt,
        &[],
    );
    let parsed_ack = TcpSegment::parse(remote.ip, local.ip, &ack_seg, true).unwrap();
    client.handle_segment_at(&parsed_ack, 1050);

    // Segment 1 [100..200] is unsacked, segments 2 and 3 are marked sacked
    assert!(!client.retransmit_queue[0].sacked);
    assert!(client.retransmit_queue[1].sacked);
    assert!(client.retransmit_queue[2].sacked);
    assert_eq!(client.stats.sack_blocks_received, 1);
}
