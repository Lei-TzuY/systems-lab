use std::str::FromStr;
use toy_tcpip::dhcpv6::{Dhcpv6Message, Dhcpv6Server, DHCPV6_CLIENT_PORT, DHCPV6_MSG_ADVERTISE, DHCPV6_MSG_SOLICIT, DHCPV6_SERVER_PORT};
use toy_tcpip::ipv6::Ipv6Address;

#[test]
fn test_dhcpv6_solicit_advertise_handshake() {
    let client_duid = vec![0x00, 0x01, 0x00, 0x01, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    let solicit = Dhcpv6Message::build_solicit(0x654321, &client_duid);
    let raw_sol = solicit.serialize();

    let parsed_sol = Dhcpv6Message::parse(&raw_sol).unwrap();
    assert_eq!(parsed_sol.msg_type, DHCPV6_MSG_SOLICIT);
    assert_eq!(parsed_sol.transaction_id, 0x654321);
    assert_eq!(DHCPV6_CLIENT_PORT, 546);
    assert_eq!(DHCPV6_SERVER_PORT, 547);

    let mut server = Dhcpv6Server::new();
    let advertise = server.handle_solicit(&parsed_sol).unwrap();
    let raw_adv = advertise.serialize();

    let parsed_adv = Dhcpv6Message::parse(&raw_adv).unwrap();
    assert_eq!(parsed_adv.msg_type, DHCPV6_MSG_ADVERTISE);
    assert_eq!(parsed_adv.transaction_id, 0x654321);

    let leased_ip = parsed_adv.get_assigned_ipv6().unwrap();
    assert_eq!(leased_ip, Ipv6Address::from_str("2001:db8::64").unwrap());
}
