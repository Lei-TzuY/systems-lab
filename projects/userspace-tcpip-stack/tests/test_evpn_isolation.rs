//! Route Target isolation between two tenants on the same pair of leaves.
//!
//! Two VNIs share the leaves, the spine, the underlay, and the single BGP
//! session that carries both tenants' routes. The only thing keeping them apart
//! is the Route Target, so this is where a mistake in RT matching, RD handling,
//! or VNI selection shows up as one tenant able to reach another.
//!
//! The topology is built here from the public API rather than by a lab helper,
//! because the asymmetry in the third tenant is the point of two of these tests
//! and a shared builder would have to be bent out of shape to express it.

use toy_tcpip::bgp_evpn::RouteTarget;
use toy_tcpip::bgp_router::BgpPeerMode;
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_vtep::OverlayDecision;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{LabRouter, VirtualLab};
use toy_tcpip::router::RouteSource;
use toy_tcpip::stack::NetStackConfig;

const RED: u32 = 5001;
const BLUE: u32 = 5002;
const AS1: u32 = 65001;
const AS2: u32 = 65002;

const VTEP1: Ipv4Address = Ipv4Address([10, 0, 0, 1]);
const VTEP2: Ipv4Address = Ipv4Address([10, 0, 0, 2]);

/// One MAC, deliberately reused in both tenants on both leaves. Two tenants each
/// numbering a VM `...:01` is ordinary, and the fabric has to cope.
const SHARED_MAC_LEFT: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
const SHARED_MAC_RIGHT: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x02]);

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address::new(a, b, c, d)
}

fn mac(a: u8, b: u8) -> MacAddress {
    MacAddress([0x02, 0x00, 0x00, 0x00, a, b])
}

fn host(m: MacAddress, addr: Ipv4Address) -> NetStackConfig {
    NetStackConfig {
        mac: m,
        ip: addr,
        ipv6: None,
        subnet_mask: 24,
        gateway: None,
    }
}

/// Two leaves, one spine, two tenants.
///
/// `leaf2_blue_import` is what leaf2 imports for the blue tenant. Passing the
/// matching RT gives a working two-tenant fabric; passing a different one is the
/// negative control: blue is configured, blue advertises, and blue still must not
/// forward, because nothing asked for those routes.
fn build_two_tenant_fabric(leaf2_blue_import: RouteTarget) -> VirtualLab {
    let red_rt = RouteTarget::as2(65001, RED);
    let blue_rt = RouteTarget::as2(65001, BLUE);

    let mut lab = VirtualLab::new();
    for link in [
        "red1",
        "blue1",
        "red2",
        "blue2",
        "leaf1spine",
        "leaf2spine",
        "lo1",
        "lo2",
    ] {
        lab.add_link(link);
    }

    // Red and blue use overlapping tenant addressing on purpose: 192.168.10.0/24
    // exists twice, and only the VNI keeps the two copies apart.
    lab.add_host("red_a", "red1", host(SHARED_MAC_LEFT, ip(192, 168, 10, 11)));
    lab.add_host(
        "red_b",
        "red2",
        host(SHARED_MAC_RIGHT, ip(192, 168, 10, 22)),
    );
    lab.add_host(
        "blue_a",
        "blue1",
        host(SHARED_MAC_LEFT, ip(192, 168, 10, 11)),
    );
    lab.add_host(
        "blue_b",
        "blue2",
        host(SHARED_MAC_RIGHT, ip(192, 168, 10, 22)),
    );

    let mut leaf1 = LabRouter::new("leaf1");
    leaf1.add_interface("eth0", mac(0x01, 0x00), ip(192, 168, 10, 1), 24, "red1");
    leaf1.add_interface("eth2", mac(0x01, 0x02), ip(192, 168, 10, 1), 24, "blue1");
    leaf1.add_interface("eth1", mac(0x01, 0x01), ip(10, 1, 0, 1), 30, "leaf1spine");
    leaf1.add_interface("lo0", mac(0x01, 0xFF), VTEP1, 32, "lo1");
    leaf1.routing_table.add_route_from(
        VTEP2,
        32,
        Some(ip(10, 1, 0, 2)),
        "eth1",
        RouteSource::Static,
    );
    leaf1.enable_bgp(AS1, ip(1, 1, 1, 1)).set_hold_time(9);
    leaf1.add_bgp_peer(VTEP2, AS2, VTEP1, BgpPeerMode::Active);
    leaf1.enable_vtep(VTEP1, "eth1");
    leaf1.add_evpn_instance(
        RED,
        RouteDistinguisher::new(VTEP1, RED as u16),
        &[red_rt],
        &[red_rt],
    );
    leaf1.add_evpn_instance(
        BLUE,
        RouteDistinguisher::new(VTEP1, BLUE as u16),
        &[blue_rt],
        &[blue_rt],
    );
    leaf1.attach_evpn_access_port(RED, "eth0");
    leaf1.attach_evpn_access_port(BLUE, "eth2");

    let mut spine = LabRouter::new("spine");
    spine.add_interface("eth0", mac(0x02, 0x00), ip(10, 1, 0, 2), 30, "leaf1spine");
    spine.add_interface("eth1", mac(0x02, 0x01), ip(10, 2, 0, 1), 30, "leaf2spine");
    spine.routing_table.add_route_from(
        VTEP1,
        32,
        Some(ip(10, 1, 0, 1)),
        "eth0",
        RouteSource::Static,
    );
    spine.routing_table.add_route_from(
        VTEP2,
        32,
        Some(ip(10, 2, 0, 2)),
        "eth1",
        RouteSource::Static,
    );

    let mut leaf2 = LabRouter::new("leaf2");
    leaf2.add_interface("eth0", mac(0x03, 0x00), ip(192, 168, 10, 2), 24, "red2");
    leaf2.add_interface("eth2", mac(0x03, 0x02), ip(192, 168, 10, 2), 24, "blue2");
    leaf2.add_interface("eth1", mac(0x03, 0x01), ip(10, 2, 0, 2), 30, "leaf2spine");
    leaf2.add_interface("lo0", mac(0x03, 0xFF), VTEP2, 32, "lo2");
    leaf2.routing_table.add_route_from(
        VTEP1,
        32,
        Some(ip(10, 2, 0, 1)),
        "eth1",
        RouteSource::Static,
    );
    leaf2.enable_bgp(AS2, ip(3, 3, 3, 3)).set_hold_time(9);
    leaf2.add_bgp_peer(VTEP1, AS1, VTEP2, BgpPeerMode::Passive);
    leaf2.enable_vtep(VTEP2, "eth1");
    leaf2.add_evpn_instance(
        RED,
        RouteDistinguisher::new(VTEP2, RED as u16),
        &[red_rt],
        &[red_rt],
    );
    leaf2.add_evpn_instance(
        BLUE,
        RouteDistinguisher::new(VTEP2, BLUE as u16),
        &[leaf2_blue_import],
        &[blue_rt],
    );
    leaf2.attach_evpn_access_port(RED, "eth0");
    leaf2.attach_evpn_access_port(BLUE, "eth2");

    lab.add_router(leaf1);
    lab.add_router(spine);
    lab.add_router(leaf2);
    lab
}

/// Brings the fabric up and makes all four hosts speak.
fn converged(leaf2_blue_import: RouteTarget) -> VirtualLab {
    let mut lab = build_two_tenant_fabric(leaf2_blue_import);
    lab.run_until(250, 60_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.carries_evpn()))
    });
    for (name, dst) in [
        ("red_a", ip(192, 168, 10, 22)),
        ("red_b", ip(192, 168, 10, 11)),
        ("blue_a", ip(192, 168, 10, 22)),
        ("blue_b", ip(192, 168, 10, 11)),
    ] {
        let frame = lab
            .host_mut(name)
            .unwrap()
            .stack
            .ping4(dst, 1, 1, b"t")
            .unwrap();
        lab.send_from_host(name, frame);
        lab.run_until(250, 30_000, |_| false);
    }
    lab.run_until(250, 30_000, |_| false);
    lab
}

fn matching() -> VirtualLab {
    converged(RouteTarget::as2(65001, BLUE))
}

// ============================================================================
// The same MAC in two tenants
// ============================================================================

#[test]
fn test_the_same_mac_in_two_vnis_is_two_independent_entries() {
    let lab = matching();
    let leaf1 = lab.router("leaf1").unwrap().vtep().unwrap();

    // The far end of each tenant uses the same MAC. Both must be learned, in
    // their own instance, without either overwriting the other.
    assert_eq!(
        leaf1.lookup_remote(RED, &SHARED_MAC_RIGHT),
        Some(VTEP2),
        "the red tenant never learned its remote MAC"
    );
    assert_eq!(
        leaf1.lookup_remote(BLUE, &SHARED_MAC_RIGHT),
        Some(VTEP2),
        "the blue tenant never learned its remote MAC"
    );

    // They are genuinely separate rows, reached by separate Route Distinguishers.
    let red_entry = leaf1
        .instance(RED)
        .unwrap()
        .remote_macs
        .get(&SHARED_MAC_RIGHT)
        .unwrap();
    let blue_entry = leaf1
        .instance(BLUE)
        .unwrap()
        .remote_macs
        .get(&SHARED_MAC_RIGHT)
        .unwrap();
    assert_eq!(red_entry.vtep, blue_entry.vtep);

    let bgp = lab.router("leaf1").unwrap().bgp().unwrap();
    let rds: Vec<String> = bgp
        .evpn_adj_rib_in
        .iter_paths()
        .filter(|p| p.route.mac() == Some(SHARED_MAC_RIGHT))
        .map(|p| p.route.key().rd().to_string())
        .collect();
    assert!(
        rds.contains(&"10.0.0.2:5001".to_string()) && rds.contains(&"10.0.0.2:5002".to_string()),
        "the two tenants' routes did not arrive under distinct RDs: {:?}",
        rds
    );
}

#[test]
fn test_a_local_mac_in_one_tenant_is_not_a_remote_mac_in_the_other() {
    let lab = matching();
    let leaf1 = lab.router("leaf1").unwrap().vtep().unwrap();

    // SHARED_MAC_LEFT is local to leaf1 in both tenants. Neither instance may
    // hold it as remote, or leaf1 would tunnel its own host's traffic away.
    for vni in [RED, BLUE] {
        assert!(
            leaf1
                .instance(vni)
                .unwrap()
                .local_macs
                .contains_key(&SHARED_MAC_LEFT),
            "VNI {} did not learn its local host",
            vni
        );
        assert_eq!(leaf1.lookup_remote(vni, &SHARED_MAC_LEFT), None);
    }
}

// ============================================================================
// Route Target matching
// ============================================================================

#[test]
fn test_a_route_only_lands_in_the_instance_whose_rt_matches() {
    let lab = matching();
    let leaf1 = lab.router("leaf1").unwrap().vtep().unwrap();

    // Every remote MAC in the red instance came from a route carrying the red
    // RT, and the same for blue. A route landing in both would be a leak.
    let bgp = lab.router("leaf1").unwrap().bgp().unwrap();
    for path in bgp.evpn_adj_rib_in.iter_paths() {
        let expected = if path.route.vni() == RED { RED } else { BLUE };
        let rt = RouteTarget::as2(65001, expected);
        assert!(
            path.route.route_targets.contains(&rt),
            "a route for VNI {} arrived without the RT that instance imports: {:?}",
            path.route.vni(),
            path.route.route_targets
        );
    }

    assert_eq!(leaf1.instance(RED).unwrap().remote_macs.len(), 1);
    assert_eq!(leaf1.instance(BLUE).unwrap().remote_macs.len(), 1);
}

#[test]
fn test_a_tenant_whose_import_rt_does_not_match_gets_no_forwarding_state() {
    // Blue is configured on both leaves, both leaves advertise it, and the
    // session carrying it is the same one red is working over. Leaf2 simply
    // imports a Route Target nobody exports.
    let lab = converged(RouteTarget::as2(65001, 9_999));

    let leaf2 = lab.router("leaf2").unwrap().vtep().unwrap();
    assert_eq!(
        leaf2.lookup_remote(RED, &SHARED_MAC_LEFT),
        Some(VTEP1),
        "red stopped working, so this test proves nothing about blue"
    );
    assert_eq!(
        leaf2.lookup_remote(BLUE, &SHARED_MAC_LEFT),
        None,
        "blue imported a route no configured Route Target asked for"
    );
    assert!(leaf2.instance(BLUE).unwrap().remote_macs.is_empty());

    // The route was refused at the edge of the Adj-RIB-In, not merely ignored
    // when programming, so it never occupied a table at all.
    let bgp = lab.router("leaf2").unwrap().bgp().unwrap();
    assert!(
        !bgp.evpn_adj_rib_in
            .iter_paths()
            .any(|p| p.route.vni() == BLUE),
        "a route for the unimported tenant was still stored"
    );
    assert!(
        bgp.peers()[0].counters.evpn_rt_rejected > 0,
        "nothing was counted as rejected by Route Target"
    );
}

#[test]
fn test_an_unimported_tenant_cannot_use_the_other_tenants_tunnel() {
    let lab = converged(RouteTarget::as2(65001, 9_999));
    let leaf2 = lab.router("leaf2").unwrap().vtep().unwrap();

    // Red resolves. Blue, asked about the very same MAC on its own access port,
    // must not fall through to red's entry.
    assert_eq!(
        leaf2.forward("eth0", SHARED_MAC_LEFT),
        OverlayDecision::Unicast {
            vni: RED,
            vtep: VTEP1
        }
    );
    let blue = leaf2.forward("eth2", SHARED_MAC_LEFT);
    assert!(
        !matches!(blue, OverlayDecision::Unicast { .. }),
        "blue resolved a MAC through red's forwarding entry: {:?}",
        blue
    );
}

#[test]
fn test_removing_an_import_rt_drops_that_tenants_state_and_leaves_the_other() {
    let mut lab = matching();
    assert!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .lookup_remote(BLUE, &SHARED_MAC_LEFT)
            .is_some()
    );

    // Withdraw the import at the speaker, as an operator changing the VRF would.
    let removed = lab
        .router_mut("leaf2")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .remove_import_route_target(&RouteTarget::as2(65001, BLUE));
    assert!(removed);
    lab.router_mut("leaf2")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .instance_mut(BLUE)
        .unwrap()
        .import_rts
        .clear();
    lab.run_until(250, 60_000, |_| false);

    let leaf2 = lab.router("leaf2").unwrap().vtep().unwrap();
    assert_eq!(
        leaf2.lookup_remote(BLUE, &SHARED_MAC_LEFT),
        None,
        "blue kept forwarding state after its import Route Target was removed"
    );
    assert_eq!(
        leaf2.lookup_remote(RED, &SHARED_MAC_LEFT),
        Some(VTEP1),
        "removing blue's import took red down with it"
    );
}

// ============================================================================
// Traffic
// ============================================================================

#[test]
fn test_both_tenants_carry_traffic_over_the_one_session_without_meeting() {
    let mut lab = matching();

    for (src, dst_ip, id) in [
        ("red_a", ip(192, 168, 10, 22), 0x11u16),
        ("blue_a", ip(192, 168, 10, 22), 0x22),
    ] {
        let frame = lab
            .host_mut(src)
            .unwrap()
            .stack
            .ping4(dst_ip, id, 5, b"tenant")
            .unwrap();
        lab.send_from_host(src, frame);
        lab.run_until(250, 60_000, |_| false);
    }

    // Each reply came back to the host that asked, in its own tenant.
    for (name, id) in [("red_a", 0x11u16), ("blue_a", 0x22)] {
        let replies = &lab.host(name).unwrap().stack.received_icmp_replies;
        assert!(
            replies.iter().any(|(_, i, s)| *i == id && *s == 5),
            "{} got no reply across its own overlay; saw {:?}",
            name,
            replies
        );
    }

    // The two tenants used different VNIs on the wire, which is the only thing
    // separating two identical /24s over one underlay.
    let leaf1 = lab.router("leaf1").unwrap().vtep().unwrap();
    assert_eq!(leaf1.vni_for_access("eth0"), Some(RED));
    assert_eq!(leaf1.vni_for_access("eth2"), Some(BLUE));
}
