use toy_tcpip::arp::{ArpOpcode, ArpPacket, ArpTable};
use toy_tcpip::ethernet::MacAddress;

#[test]
fn test_arp_request_and_reply_fields() {
    let host_a_mac = MacAddress([0x00, 0x0c, 0x29, 0x11, 0x22, 0x33]);
    let host_a_ip = [192, 168, 10, 5];

    let host_b_mac = MacAddress([0x00, 0x0c, 0x29, 0xaa, 0xbb, 0xcc]);
    let host_b_ip = [192, 168, 10, 1];

    // Host A sends ARP Request: "Who has 192.168.10.1?"
    let req = ArpPacket::build_request(host_a_mac, host_a_ip, host_b_ip);
    let req_bytes = req.serialize();

    let parsed_req = ArpPacket::parse(&req_bytes).unwrap();
    assert_eq!(parsed_req.opcode, ArpOpcode::Request);
    assert_eq!(parsed_req.sender_mac, host_a_mac);
    assert_eq!(parsed_req.sender_ip, host_a_ip);
    assert_eq!(parsed_req.target_ip, host_b_ip);

    // Host B answers with ARP Reply
    let reply = ArpPacket::build_reply(host_b_mac, host_b_ip, host_a_mac, host_a_ip);
    let reply_bytes = reply.serialize();

    let parsed_reply = ArpPacket::parse(&reply_bytes).unwrap();
    assert_eq!(parsed_reply.opcode, ArpOpcode::Reply);
    assert_eq!(parsed_reply.sender_mac, host_b_mac);
    assert_eq!(parsed_reply.sender_ip, host_b_ip);
    assert_eq!(parsed_reply.target_mac, host_a_mac);
    assert_eq!(parsed_reply.target_ip, host_a_ip);
}

#[test]
fn test_arp_table_operations() {
    let mut table = ArpTable::new();
    let ip1 = [10, 0, 0, 1];
    let ip2 = [10, 0, 0, 2];
    let mac1 = MacAddress([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
    let mac2 = MacAddress([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);

    table.insert(ip1, mac1);
    table.insert(ip2, mac2);

    assert_eq!(table.lookup(&ip1), Some(mac1));
    assert_eq!(table.lookup(&ip2), Some(mac2));
    assert_eq!(table.lookup(&[10, 0, 0, 3]), None);

    assert_eq!(table.remove(&ip1), Some(mac1));
    assert_eq!(table.lookup(&ip1), None);
}
