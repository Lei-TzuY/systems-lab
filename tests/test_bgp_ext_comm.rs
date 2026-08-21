use toy_tcpip::bgp_ext_comm::{
    BgpExtCommunityContainer, BgpExtendedCommunity, TUNNEL_TYPE_GENEVE, TUNNEL_TYPE_MPLS,
    TUNNEL_TYPE_SRV6, TUNNEL_TYPE_VXLAN,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_bgp_extended_communities_serialization_and_helpers() {
    let rt_as = BgpExtendedCommunity::RouteTarget2Octet {
        asn: 64512,
        value: 500,
    };
    let rt_ip = BgpExtendedCommunity::RouteTargetIpv4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        value: 100,
    };
    let soo = BgpExtendedCommunity::RouteOrigin2Octet {
        asn: 64512,
        value: 99,
    };
    let color = BgpExtendedCommunity::Color {
        flags: 0,
        color: 1000,
    };
    let vxlan = BgpExtendedCommunity::TunnelEncapsulation {
        tunnel_type: TUNNEL_TYPE_VXLAN,
    };

    assert_eq!(
        BgpExtendedCommunity::parse(&rt_as.serialize()),
        Some(rt_as.clone())
    );
    assert_eq!(
        BgpExtendedCommunity::parse(&rt_ip.serialize()),
        Some(rt_ip.clone())
    );
    assert_eq!(
        BgpExtendedCommunity::parse(&soo.serialize()),
        Some(soo.clone())
    );
    assert_eq!(
        BgpExtendedCommunity::parse(&color.serialize()),
        Some(color.clone())
    );
    assert_eq!(
        BgpExtendedCommunity::parse(&vxlan.serialize()),
        Some(vxlan.clone())
    );

    let mut container = BgpExtCommunityContainer::new();
    container.add(rt_as);
    container.add(color);
    container.add(vxlan);

    assert_eq!(container.get_color(), Some(1000));
    assert_eq!(container.get_tunnel_encap(), Some(TUNNEL_TYPE_VXLAN));
}

#[test]
fn test_bgp_tunnel_constants() {
    assert_eq!(TUNNEL_TYPE_VXLAN, 8);
    assert_eq!(TUNNEL_TYPE_MPLS, 10);
    assert_eq!(TUNNEL_TYPE_GENEVE, 19);
    assert_eq!(TUNNEL_TYPE_SRV6, 27);
}
