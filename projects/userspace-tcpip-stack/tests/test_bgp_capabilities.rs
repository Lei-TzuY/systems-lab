//! OPEN capability negotiation and 4-octet autonomous system numbers.
//!
//! The capability set decides what a session is allowed to carry, so the tests
//! that matter most here are the negative ones: a neighbour that did not ask for
//! EVPN must never be sent an EVPN route, whatever the local RIB holds.
//!
//! The 32-bit ASN tests run the same fabric on ASNs above 65535, because that is
//! the only way to catch a truncation - 4200000001 and 4200000002 both narrow to
//! values that are not merely wrong but belong to somebody else.

mod common;

use common::bgp_lab::{RawBgpPeer, ip, prefix};
use toy_tcpip::bgp::{AS_TRANS, BgpOpenMessage, BgpPdu};
use toy_tcpip::bgp_caps::{
    AfiSafi, BGP_CAP_FOUR_OCTET_AS, BGP_CAP_MULTIPROTOCOL, BGP_OPT_PARAM_CAPABILITY, BgpCapability,
    BgpCapabilitySet, negotiate,
};
use toy_tcpip::bgp_router::BgpState;
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{build_evpn_fabric, evpn_rt};

const AS1: u32 = 65001;
const AS2: u32 = 65002;
/// Both above 65535, and both narrowing to something that is a real, different AS.
const AS4_LEFT: u32 = 4_200_000_001;
const AS4_RIGHT: u32 = 4_200_000_002;

const VNI: u32 = 5001;
const MAC_A: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x0A]);
const VTEP1: Ipv4Address = Ipv4Address([10, 0, 0, 1]);
const VTEP2: Ipv4Address = Ipv4Address([10, 0, 0, 2]);

// ============================================================================
// What the speaker offers
// ============================================================================

#[test]
fn test_a_plain_speaker_offers_ipv4_unicast_and_four_octet_as() {
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.establish();

    let caps = peer.victim_bgp().local_capabilities();
    assert!(caps.supports(AfiSafi::IPV4_UNICAST));
    assert!(
        !caps.supports(AfiSafi::L2VPN_EVPN),
        "a router with no VTEP offered EVPN"
    );
    // AS4 is offered unconditionally, even for a 16-bit local ASN: it is what
    // lets the session use the wide AS_PATH for a path that crossed a 32-bit AS
    // somewhere else.
    assert_eq!(caps.four_octet_as(), Some(AS1));
}

#[test]
fn test_enabling_a_vtep_adds_evpn_to_what_the_speaker_offers() {
    let mut peer = RawBgpPeer::connect_configured(AS1, AS2, ip(9, 9, 9, 9), |r| {
        r.enable_vtep(VTEP1, "eth0");
    });
    peer.establish();
    assert!(
        peer.victim_bgp()
            .local_capabilities()
            .supports(AfiSafi::L2VPN_EVPN)
    );
}

#[test]
fn test_the_open_on_the_wire_really_carries_the_capability_parameter() {
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.establish();

    // `establish` never reads, so the router's own OPEN is still queued.
    let open = peer
        .drain()
        .into_iter()
        .find_map(|m| match m {
            BgpPdu::Open(o) => Some(o),
            _ => None,
        })
        .expect("the router sent no OPEN");

    assert_eq!(
        open.opt_params[0], BGP_OPT_PARAM_CAPABILITY,
        "the optional parameter block is not a capability list"
    );
    assert_eq!(
        open.opt_params[1] as usize,
        open.opt_params.len() - 2,
        "the parameter length does not describe what follows it"
    );
    let decoded = open.capabilities().unwrap();
    assert!(decoded.supports(AfiSafi::IPV4_UNICAST));
    assert_eq!(decoded.four_octet_as(), Some(AS1));

    // And the session negotiated from what was actually on the wire.
    let negotiated = &peer.victim_bgp().peers()[0].negotiated;
    assert!(negotiated.supports(AfiSafi::IPV4_UNICAST));
    assert!(negotiated.four_octet_as);
}

// ============================================================================
// Negotiation against different kinds of neighbour
// ============================================================================

#[test]
fn test_a_legacy_neighbour_negotiates_ipv4_unicast_and_nothing_else() {
    let mut peer = RawBgpPeer::connect_configured(AS1, AS2, ip(9, 9, 9, 9), |r| {
        r.enable_vtep(VTEP1, "eth0");
    });
    // No capabilities at all: a plain RFC 4271 speaker.
    peer.establish_legacy();
    assert_eq!(peer.state(), BgpState::Established);

    let negotiated = &peer.victim_bgp().peers()[0].negotiated;
    assert!(negotiated.supports(AfiSafi::IPV4_UNICAST));
    assert!(
        !negotiated.supports_evpn(),
        "EVPN was negotiated with a peer that never mentioned it"
    );
    assert!(
        !negotiated.four_octet_as,
        "4-octet ASNs were assumed for a peer that did not advertise them"
    );
    assert!(!peer.victim_bgp().peers()[0].carries_evpn());
}

#[test]
fn test_a_neighbour_offering_only_ipv4_does_not_get_evpn() {
    let mut ipv4_only = BgpCapabilitySet::new();
    ipv4_only.advertise(AfiSafi::IPV4_UNICAST);
    ipv4_only.push(BgpCapability::FourOctetAs(AS2));

    let mut peer = RawBgpPeer::connect_configured(AS1, AS2, ip(9, 9, 9, 9), |r| {
        r.enable_vtep(VTEP1, "eth0");
    });
    peer.establish_with(&ipv4_only);

    let negotiated = &peer.victim_bgp().peers()[0].negotiated;
    assert!(negotiated.four_octet_as);
    assert!(!negotiated.supports_evpn());
}

#[test]
fn test_an_unknown_capability_does_not_prevent_the_session() {
    let mut odd = BgpCapabilitySet::new();
    odd.advertise(AfiSafi::IPV4_UNICAST);
    odd.push(BgpCapability::Unknown {
        code: 199,
        value: vec![0xDE, 0xAD, 0xBE, 0xEF],
    });

    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.establish_with(&odd);
    assert_eq!(peer.state(), BgpState::Established);

    // It was kept for diagnostics rather than discarded or acted on.
    let seen = &peer.victim_bgp().peers()[0].negotiated.peer;
    assert!(seen.capabilities.iter().any(|c| c.code() == 199));
}

// ============================================================================
// EVPN is never sent to a peer that did not ask for it
// ============================================================================

#[test]
fn test_a_leaf_with_evpn_routes_sends_none_to_a_legacy_neighbour() {
    let mut peer = RawBgpPeer::connect_configured(AS1, AS2, ip(9, 9, 9, 9), |r| {
        // The access port has to be an interface other than the one carrying the
        // session: an EVPN access port is in the tenant bridge domain, so
        // everything arriving on it is tenant traffic, BGP included.
        r.add_interface(
            "eth1",
            MacAddress([0x02, 0, 0, 0, 0xBB, 0x02]),
            ip(192, 168, 10, 1),
            24,
            "tenant",
        );
        r.enable_vtep(VTEP1, "eth0");
        r.add_evpn_instance(
            VNI,
            RouteDistinguisher::new(VTEP1, VNI as u16),
            &[evpn_rt(65001, VNI)],
            &[evpn_rt(65001, VNI)],
        );
        r.attach_evpn_access_port(VNI, "eth1");
    });
    peer.establish_legacy();

    // Give the leaf something worth advertising, and something ordinary too.
    peer.lab
        .router_mut("victim")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .learn_local("eth1", MAC_A, Some(ip(192, 168, 10, 11)));
    peer.lab
        .router_mut("victim")
        .unwrap()
        .originate_bgp_prefix(prefix(10, 1, 0, 0, 24));
    peer.run_until(30_000, |_| false);

    let msgs = peer.drain();
    assert!(
        msgs.iter()
            .any(|m| matches!(m, BgpPdu::Update(u) if !u.nlri.is_empty())),
        "the IPv4 route was not advertised either, so this proves nothing"
    );
    for m in &msgs {
        if let BgpPdu::Update(u) = m {
            assert!(
                u.mp_reach().is_none() && u.mp_unreach().is_none(),
                "an EVPN attribute was sent to a peer that never negotiated the family"
            );
        }
    }

    // The routes exist locally; they simply have nowhere to go.
    let bgp = peer.victim_bgp();
    let (_, loc, originated) = bgp.evpn_route_counts();
    assert!(
        originated > 0 && loc > 0,
        "the leaf originated no EVPN route"
    );
    assert_eq!(bgp.evpn_adj_rib_out.route_count(peer.peer), 0);
    assert_eq!(bgp.peers()[0].counters.evpn_advertised, 0);
}

#[test]
fn test_evpn_nlri_from_an_unnegotiated_peer_is_a_protocol_violation() {
    use toy_tcpip::bgp::{AsPath, BgpOrigin, BgpPathAttributes, BgpUpdateMessage};
    use toy_tcpip::bgp_evpn::encode_evpn_nlri_list;
    use toy_tcpip::bgp_mp::MpReachNlri;
    use toy_tcpip::evpn::EvpnNlri;

    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.establish_legacy();
    assert_eq!(peer.state(), BgpState::Established);

    let nlri = encode_evpn_nlri_list(&[EvpnNlri::build_mac_ip(
        RouteDistinguisher::new(VTEP2, VNI as u16),
        MAC_A,
        None,
        VNI,
    )]);
    let mut attrs =
        BgpPathAttributes::new(BgpOrigin::Igp, AsPath::sequence(vec![AS2]), ip(0, 0, 0, 0));
    attrs.mp_reach = Some(MpReachNlri::with_ipv4_next_hop(
        AfiSafi::L2VPN_EVPN,
        VTEP2,
        nlri,
    ));
    peer.write_pdu(BgpPdu::Update(BgpUpdateMessage::mp_announce(attrs)));

    let note = peer
        .notification()
        .expect("no NOTIFICATION for EVPN NLRI on a session that never negotiated it");
    assert_eq!(note.error_code, toy_tcpip::bgp::BGP_ERR_UPDATE_MESSAGE);
    assert_ne!(peer.state(), BgpState::Established);
    assert_eq!(peer.victim_bgp().evpn_adj_rib_in.total_routes(), 0);
}

// ============================================================================
// 32-bit autonomous system numbers
// ============================================================================

#[test]
fn test_the_whole_fabric_works_on_asns_above_65535() {
    let mut lab = build_evpn_fabric(AS4_LEFT, AS4_RIGHT);
    assert!(
        lab.run_until(250, 60_000, |l| l
            .routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.carries_evpn()))),
        "the session never came up on 32-bit ASNs"
    );

    let bgp = lab.router("leaf1").unwrap().bgp().unwrap();
    assert_eq!(bgp.local_as, AS4_LEFT);
    assert_eq!(bgp.peers()[0].remote_as, AS4_RIGHT);
    assert!(bgp.peers()[0].negotiated.four_octet_as);

    // The overlay converges the same way it does on 16-bit ASNs.
    let frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(ip(192, 168, 10, 22), 1, 1, b"as4")
        .unwrap();
    lab.send_from_host("host_a", frame);
    lab.run_until(250, 60_000, |_| false);

    assert_eq!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .lookup_remote(VNI, &MAC_A),
        Some(VTEP1)
    );
}

#[test]
fn test_a_32_bit_asn_survives_the_as_path_intact() {
    let mut lab = build_evpn_fabric(AS4_LEFT, AS4_RIGHT);
    lab.run_until(250, 60_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.carries_evpn()))
    });
    let frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(ip(192, 168, 10, 22), 1, 1, b"as4")
        .unwrap();
    lab.send_from_host("host_a", frame);
    lab.run_until(250, 60_000, |_| false);

    let path = lab
        .router("leaf2")
        .unwrap()
        .bgp()
        .unwrap()
        .evpn_adj_rib_in
        .iter_paths()
        .find(|p| p.route.mac() == Some(MAC_A))
        .expect("leaf2 learned nothing");

    // The exact value, not AS_TRANS and not a truncation.
    assert_eq!(path.as_path.flatten(), vec![AS4_LEFT]);
    assert_ne!(path.as_path.flatten()[0], AS_TRANS as u32);
    assert_ne!(path.as_path.flatten()[0], AS4_LEFT & 0xFFFF);
    assert_eq!(path.peer_as, AS4_LEFT);
}

#[test]
fn test_loop_detection_works_on_32_bit_asns() {
    use toy_tcpip::bgp::AsPath;

    // A path already carrying the local AS must be refused, and the comparison
    // has to happen at full width: 4200000001 and 4200000001 & 0xFFFF are
    // different autonomous systems and must not be confused.
    let path = AsPath::sequence(vec![AS4_RIGHT, AS4_LEFT]);
    assert!(path.contains(AS4_LEFT));
    assert!(!path.contains(AS4_LEFT & 0xFFFF));
    assert!(path.needs_four_octets());

    let mut prepended = AsPath::sequence(vec![AS4_RIGHT]);
    prepended.prepend(AS4_LEFT);
    assert_eq!(prepended.flatten(), vec![AS4_LEFT, AS4_RIGHT]);
    assert_eq!(prepended.length(), 2);
}

#[test]
fn test_a_32_bit_asn_is_never_narrowed_to_a_different_real_as() {
    // The two-octet field cannot hold it, so it must hold the reserved
    // placeholder and the capability must carry the truth.
    let mut caps = BgpCapabilitySet::new();
    caps.advertise(AfiSafi::IPV4_UNICAST);
    caps.push(BgpCapability::FourOctetAs(AS4_LEFT));
    let open = BgpOpenMessage::with_capabilities(AS4_LEFT, 90, ip(1, 1, 1, 1), &caps);

    assert_eq!(open.my_as, AS_TRANS);
    assert_ne!(open.my_as as u32, AS4_LEFT & 0xFFFF);
    assert_eq!(open.effective_as(&open.capabilities().unwrap()), AS4_LEFT);

    // A 16-bit ASN is unaffected: the field still carries it directly.
    let small = BgpOpenMessage::with_capabilities(AS1, 90, ip(1, 1, 1, 1), &caps);
    assert_eq!(small.my_as, AS1 as u16);
}

#[test]
fn test_as4_path_reconstructs_the_wide_path_for_a_narrow_session() {
    use toy_tcpip::bgp::{AsPath, BgpOrigin, BgpPathAttributes, BgpUpdateMessage};

    // Encoded for a peer that did not negotiate AS4: the AS_PATH narrows the
    // 32-bit hop to AS_TRANS and AS4_PATH carries what it really was.
    let wide = AsPath::sequence(vec![AS1, AS4_RIGHT]);
    let mut attrs = BgpPathAttributes::new(BgpOrigin::Igp, wide.clone(), ip(10, 0, 0, 1));
    attrs.four_octet_as = false;
    let update = BgpUpdateMessage::announce(attrs, vec![prefix(198, 51, 100, 0, 24)]);
    let raw = BgpPdu::Update(update).serialize();

    // A 2-octet reader puts the two attributes back together and recovers the
    // original path rather than believing AS_TRANS.
    let BgpPdu::Update(decoded) = BgpPdu::parse_width(&raw, false).unwrap() else {
        panic!("not an UPDATE");
    };
    assert_eq!(
        decoded.attributes.unwrap().as_path.flatten(),
        vec![AS1, AS4_RIGHT]
    );
}

#[test]
fn test_a_peer_claiming_a_32_bit_asn_without_the_capability_is_refused() {
    // The router is configured to expect a 32-bit neighbour. An OPEN with only
    // the two-octet field cannot honestly claim to be it.
    let mut peer = RawBgpPeer::connect(AS1, AS4_RIGHT, ip(9, 9, 9, 9));
    let mut caps = BgpCapabilitySet::new();
    caps.advertise(AfiSafi::IPV4_UNICAST);
    peer.four_octet_as = false;
    peer.write(
        &BgpPdu::Open(BgpOpenMessage::with_capabilities(
            AS4_RIGHT,
            9,
            ip(5, 5, 5, 5),
            &caps,
        ))
        .serialize(),
    );

    let note = peer
        .notification()
        .expect("a peer that could not name its own ASN was accepted");
    assert_eq!(note.error_code, toy_tcpip::bgp::BGP_ERR_OPEN_MESSAGE);
    assert_ne!(peer.state(), BgpState::Established);
}

#[test]
fn test_negotiation_is_a_plain_intersection() {
    // The unit-level statement of what every session test above depends on.
    let mut both = BgpCapabilitySet::new();
    both.advertise(AfiSafi::IPV4_UNICAST);
    both.advertise(AfiSafi::L2VPN_EVPN);
    both.push(BgpCapability::FourOctetAs(AS4_LEFT));

    let mut evpn_only = BgpCapabilitySet::new();
    evpn_only.advertise(AfiSafi::L2VPN_EVPN);

    let n = negotiate(&both, &evpn_only);
    assert!(n.supports(AfiSafi::L2VPN_EVPN));
    assert!(!n.supports(AfiSafi::IPV4_UNICAST));
    assert!(!n.four_octet_as);

    // And the wire encoding is the one the constants describe.
    let raw = both.encode_opt_params();
    assert_eq!(raw[0], BGP_OPT_PARAM_CAPABILITY);
    assert!(raw.contains(&BGP_CAP_MULTIPROTOCOL));
    assert!(raw.contains(&BGP_CAP_FOUR_OCTET_AS));
}
