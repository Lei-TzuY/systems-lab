use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::tcp::{
    SocketAddrV4, TcpConnectionKey, TcpFlags, TcpManager, TcpSegment, TcpState,
};

#[test]
fn test_tcp_flags_formatting_and_bits() {
    let syn_ack = TcpFlags::syn_ack();
    assert!(syn_ack.syn);
    assert!(syn_ack.ack);
    assert!(!syn_ack.fin);
    assert_eq!(format!("{}", syn_ack), "[SYN|ACK]");

    let rst = TcpFlags::rst();
    assert_eq!(format!("{}", rst), "[RST]");
}

#[test]
fn test_tcp_full_lifecycle_handshake_data_fin() {
    let mut mgr = TcpManager::new();
    let port = 8080;
    mgr.listen(port);

    let client_ip = Ipv4Address::new(10, 0, 0, 50);
    let server_ip = Ipv4Address::new(10, 0, 0, 1);
    let client_port = 60000;

    let key = TcpConnectionKey {
        local: SocketAddrV4 { ip: server_ip, port },
        remote: SocketAddrV4 { ip: client_ip, port: client_port },
    };

    // 1. Client sends SYN (Seq 1000)
    let syn = TcpSegment::serialize(
        client_ip,
        server_ip,
        client_port,
        port,
        1000,
        0,
        TcpFlags::syn(),
        65535,
        &[],
    );
    let syn_parsed = TcpSegment::parse(client_ip, server_ip, &syn, true).unwrap();
    let syn_ack_raw = mgr.process_segment(client_ip, server_ip, &syn_parsed).expect("SYN-ACK");
    let syn_ack = TcpSegment::parse(server_ip, client_ip, &syn_ack_raw, true).unwrap();

    assert!(syn_ack.flags.syn && syn_ack.flags.ack);
    assert_eq!(syn_ack.ack_num, 1001);

    // 2. Client sends ACK (Seq 1001, Ack 1001)
    let ack = TcpSegment::serialize(
        client_ip,
        server_ip,
        client_port,
        port,
        1001,
        syn_ack.seq_num + 1,
        TcpFlags::ack(),
        65535,
        &[],
    );
    let ack_parsed = TcpSegment::parse(client_ip, server_ip, &ack, true).unwrap();
    let resp = mgr.process_segment(client_ip, server_ip, &ack_parsed);
    assert!(resp.is_none());
    assert_eq!(mgr.connections.get(&key).unwrap().state, TcpState::Established);

    // 3. Client sends 10 bytes of Data
    let data_payload = b"0123456789";
    let data_seg = TcpSegment::serialize(
        client_ip,
        server_ip,
        client_port,
        port,
        1001,
        syn_ack.seq_num + 1,
        TcpFlags { psh: true, ack: true, ..Default::default() },
        65535,
        data_payload,
    );
    let data_parsed = TcpSegment::parse(client_ip, server_ip, &data_seg, true).unwrap();
    let data_ack_raw = mgr.process_segment(client_ip, server_ip, &data_parsed).expect("Data ACK");
    let data_ack = TcpSegment::parse(server_ip, client_ip, &data_ack_raw, true).unwrap();

    assert_eq!(data_ack.ack_num, 1001 + 10);
    assert_eq!(mgr.connections.get(&key).unwrap().rx_buffer, data_payload);

    // 4. Client sends FIN
    let fin_seg = TcpSegment::serialize(
        client_ip,
        server_ip,
        client_port,
        port,
        1011,
        syn_ack.seq_num + 1,
        TcpFlags::fin_ack(),
        65535,
        &[],
    );
    let fin_parsed = TcpSegment::parse(client_ip, server_ip, &fin_seg, true).unwrap();
    let fin_ack_raw = mgr.process_segment(client_ip, server_ip, &fin_parsed).expect("FIN-ACK");
    let fin_ack = TcpSegment::parse(server_ip, client_ip, &fin_ack_raw, true).unwrap();
    assert!(fin_ack.flags.fin && fin_ack.flags.ack);
}
