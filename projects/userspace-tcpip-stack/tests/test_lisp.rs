use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lisp::{
    LISP_CONTROL_PORT, LISP_DATA_PORT, LISP_MSG_MAP_REPLY, LISP_MSG_MAP_REQUEST, LispDataPacket,
    LispMapReply, LispMapRequest, LispMapResolver,
};

#[test]
fn test_lisp_eid_rloc_resolution_and_data_plane() {
    let eid = Ipv4Address::new(10, 20, 30, 40);
    let rloc1 = Ipv4Address::new(198, 51, 100, 10);
    let rloc2 = Ipv4Address::new(198, 51, 100, 20);

    let mut resolver = LispMapResolver::new();
    resolver.register_eid(eid, rloc1, 1, 100);
    resolver.register_eid(eid, rloc2, 2, 50);

    // Control Plane: Map-Request -> Map-Reply
    let req = LispMapRequest::build(
        0xAABBCCDDEEFF0011,
        Ipv4Address::new(10, 0, 0, 1),
        Ipv4Address::new(192, 0, 2, 1),
        eid,
    );
    let raw_req = req.serialize();

    let parsed_req = LispMapRequest::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.nonce, 0xAABBCCDDEEFF0011);
    assert_eq!(parsed_req.target_eid, eid);

    let reply = resolver.resolve(&parsed_req).unwrap();
    let raw_reply = reply.serialize();

    let parsed_reply = LispMapReply::parse(&raw_reply).unwrap();
    assert_eq!(parsed_reply.target_eid, eid);
    assert_eq!(parsed_reply.locators.len(), 2);
    assert_eq!(parsed_reply.locators[0].rloc_ip, rloc1);
    assert_eq!(parsed_reply.locators[0].priority, 1);
    assert_eq!(parsed_reply.locators[1].rloc_ip, rloc2);
    assert_eq!(parsed_reply.locators[1].priority, 2);

    // Data Plane: LISP Encapsulation
    let payload = b"VM-to-VM direct overlay IP packet";
    let data_pkt = LispDataPacket::encapsulate(0x00FEDCBA, 0x00000003, payload);
    let raw_data = data_pkt.serialize();

    let parsed_data = LispDataPacket::parse(&raw_data).unwrap();
    assert_eq!(parsed_data.header.nonce, 0x00FEDCBA);
    assert_eq!(parsed_data.header.lsb, 0x00000003);
    assert_eq!(&parsed_data.inner_payload, payload);

    assert_eq!(LISP_DATA_PORT, 4341);
    assert_eq!(LISP_CONTROL_PORT, 4342);
    assert_eq!(LISP_MSG_MAP_REQUEST, 1);
    assert_eq!(LISP_MSG_MAP_REPLY, 2);
}
