use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::rsvp::{RsvpObject, RsvpPacket, IP_PROTO_RSVP, RSVP_MSG_PATH, RSVP_MSG_RESV};

#[test]
fn test_rsvp_path_and_explicit_route_object() {
    let src = Ipv4Address::new(192, 168, 10, 1);
    let dst = Ipv4Address::new(192, 168, 20, 1);
    let ero = vec![
        (false, Ipv4Address::new(10, 1, 1, 1)),
        (false, Ipv4Address::new(10, 2, 2, 2)),
        (false, dst),
    ];

    let path_pkt = RsvpPacket::build_path(src, dst, 401, 1, 100_000_000, &ero);
    let raw = path_pkt.serialize();

    let parsed = RsvpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.msg_type, RSVP_MSG_PATH);
    assert_eq!(parsed.objects.len(), 5);

    if let RsvpObject::Session { dest_ip, tunnel_id, ext_tunnel_id } = &parsed.objects[0] {
        assert_eq!(*dest_ip, dst);
        assert_eq!(*tunnel_id, 401);
        assert_eq!(*ext_tunnel_id, src);
    } else {
        panic!("Expected SESSION object");
    }

    if let RsvpObject::ExplicitRoute { hops } = &parsed.objects[4] {
        assert_eq!(hops.len(), 3);
        assert_eq!(hops[1].1, Ipv4Address::new(10, 2, 2, 2));
    } else {
        panic!("Expected ERO object");
    }

    assert_eq!(IP_PROTO_RSVP, 46);
}

#[test]
fn test_rsvp_resv_label_allocation() {
    let src = Ipv4Address::new(192, 168, 10, 1);
    let dst = Ipv4Address::new(192, 168, 20, 1);
    let resv_pkt = RsvpPacket::build_resv(src, dst, 401, 1048);
    let raw = resv_pkt.serialize();

    let parsed = RsvpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.msg_type, RSVP_MSG_RESV);

    if let RsvpObject::Label { label } = &parsed.objects[1] {
        assert_eq!(*label, 1048);
    } else {
        panic!("Expected LABEL object");
    }
}
