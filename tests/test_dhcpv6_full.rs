use toy_tcpip::dhcpv6::*;
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv6::Ipv6Address;

#[test]
fn test_dhcpv6_duid_formats() {
    // 1. DUID-LL
    let mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    let duid_ll = Duid::new_ll(mac);
    let raw_ll = duid_ll.serialize();
    let parsed_ll = Duid::parse(&raw_ll).unwrap();
    assert_eq!(duid_ll, parsed_ll);

    // 2. DUID-LLT
    let duid_llt = Duid::new_llt(mac, 0x12345678);
    let raw_llt = duid_llt.serialize();
    let parsed_llt = Duid::parse(&raw_llt).unwrap();
    assert_eq!(duid_llt, parsed_llt);

    // 3. DUID-UUID
    let uuid = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let duid_uuid = Duid::new_uuid(uuid);
    let raw_uuid = duid_uuid.serialize();
    let parsed_uuid = Duid::parse(&raw_uuid).unwrap();
    assert_eq!(duid_uuid, parsed_uuid);
}

#[test]
fn test_dhcpv6_four_message_exchange_and_prefix_delegation() {
    let mac = MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    let client_duid = Duid::new_ll(mac);
    let mut client = Dhcpv6Client::new(client_duid.clone());
    let mut server = Dhcpv6Server::new();

    // 1. Client sends Solicit (4-message, with PD request)
    let solicit_raw = client.start_solicit(false, true, 1000);
    assert_eq!(client.state, Dhcpv6ClientState::Soliciting);

    let parsed_solicit = Dhcpv6Message::parse(&solicit_raw).unwrap();
    assert_eq!(parsed_solicit.msg_type, DHCPV6_MSG_SOLICIT);

    // 2. Server generates Advertise
    let advertise_msg = server.handle_solicit(&parsed_solicit).unwrap();
    assert_eq!(advertise_msg.msg_type, DHCPV6_MSG_ADVERTISE);
    assert!(advertise_msg.get_assigned_ipv6().is_some());
    assert!(advertise_msg.get_delegated_prefix().is_some());

    let advertise_raw = advertise_msg.serialize();

    // 3. Client handles Advertise and produces Request
    let request_raw = client.handle_advertise(&advertise_raw, 1050).unwrap();
    assert_eq!(client.state, Dhcpv6ClientState::Requesting);

    let parsed_request = Dhcpv6Message::parse(&request_raw).unwrap();
    assert_eq!(parsed_request.msg_type, DHCPV6_MSG_REQUEST);

    // 4. Server handles Request and produces Reply
    let reply_msg = server.handle_request(&parsed_request).unwrap();
    assert_eq!(reply_msg.msg_type, DHCPV6_MSG_REPLY);

    let reply_raw = reply_msg.serialize();

    // 5. Client handles Reply and transitions to Bound
    let ok = client.handle_reply(&reply_raw, 1100);
    assert!(ok);
    assert_eq!(client.state, Dhcpv6ClientState::Bound);
    assert!(client.assigned_ip.is_some());
    assert!(client.delegated_prefix.is_some());
    assert!(!client.dns_servers.is_empty());
    assert!(!client.search_list.is_empty());

    // 6. Client releases lease
    let release_raw = client.create_release().unwrap();
    assert_eq!(client.state, Dhcpv6ClientState::Init);
    let parsed_release = Dhcpv6Message::parse(&release_raw).unwrap();
    assert_eq!(parsed_release.msg_type, DHCPV6_MSG_RELEASE);

    let server_ack = server.handle_release(&parsed_release).unwrap();
    assert_eq!(server_ack.msg_type, DHCPV6_MSG_REPLY);
    assert_eq!(server.active_leases.len(), 0);
}

#[test]
fn test_dhcpv6_rapid_commit_two_message_exchange() {
    let mac = MacAddress([0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]);
    let client_duid = Duid::new_ll(mac);
    let mut client = Dhcpv6Client::new(client_duid);
    let mut server = Dhcpv6Server::new();

    // 1. Client sends Solicit with Rapid Commit
    let solicit_raw = client.start_solicit(true, true, 1000);
    let parsed_solicit = Dhcpv6Message::parse(&solicit_raw).unwrap();
    assert!(parsed_solicit.has_rapid_commit());

    // 2. Server immediately answers with Reply
    let reply_msg = server.handle_solicit(&parsed_solicit).unwrap();
    assert_eq!(reply_msg.msg_type, DHCPV6_MSG_REPLY);
    assert!(reply_msg.has_rapid_commit());

    let reply_raw = reply_msg.serialize();

    // 3. Client processes Reply directly into Bound state
    let ok = client.handle_reply(&reply_raw, 1020);
    assert!(ok);
    assert_eq!(client.state, Dhcpv6ClientState::Bound);
    assert_eq!(
        client.assigned_ip.unwrap(),
        Ipv6Address::new([0x2001, 0x0db8, 0, 0, 0, 0, 0, 100])
    );
    let (prefix, len) = client.delegated_prefix.unwrap();
    assert_eq!(len, 64);
    assert_eq!(
        prefix,
        Ipv6Address::new([0x2001, 0x0db8, 0xcafe, 0x0001, 0, 0, 0, 0])
    );
}
