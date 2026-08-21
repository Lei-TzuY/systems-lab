use toy_tcpip::bgp::{BGP_HEADER_LEN, BgpMessage, BgpRib};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_bgp_open_message_serialization() {
    let open = BgpMessage::build_open(64512, 90, Ipv4Address::new(172, 16, 1, 1));
    let raw = open.serialize();

    assert_eq!(raw.len(), BGP_HEADER_LEN + 10);
    let parsed = BgpMessage::parse(&raw).unwrap();
    if let BgpMessage::Open {
        version,
        my_as,
        hold_time,
        bgp_id,
    } = parsed
    {
        assert_eq!(version, 4);
        assert_eq!(my_as, 64512);
        assert_eq!(hold_time, 90);
        assert_eq!(bgp_id, Ipv4Address::new(172, 16, 1, 1));
    } else {
        panic!("Expected BGP Open message");
    }
}

#[test]
fn test_bgp_update_and_rib_insertion() {
    let mut rib = BgpRib::new();
    let prefix = Ipv4Address::new(198, 51, 100, 0);
    let mask = 24;
    let next_hop = Ipv4Address::new(203, 0, 113, 254);
    let as_path = vec![65001, 65002, 65003];

    rib.insert(prefix, mask, next_hop, as_path.clone());

    let update = BgpMessage::build_update(prefix, mask, next_hop, as_path.clone());
    let raw = update.serialize();
    let parsed = BgpMessage::parse(&raw).unwrap();

    if let BgpMessage::Update {
        as_path: p,
        next_hop: nh,
        nlri_prefix: pr,
        nlri_mask: m,
    } = parsed
    {
        assert_eq!(p, as_path);
        assert_eq!(nh, next_hop);
        assert_eq!(pr, prefix);
        assert_eq!(m, mask);
    } else {
        panic!("Expected BGP Update message");
    }
}
