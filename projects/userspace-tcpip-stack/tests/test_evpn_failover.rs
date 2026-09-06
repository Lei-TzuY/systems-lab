//! What happens to the overlay when the control plane changes its mind.
//!
//! Three ways a remote MAC stops being where it was - the host disappears, the
//! session carrying the route dies, or the host turns up somewhere else - and in
//! all three the old VXLAN forwarding state has to go. A stale entry here is not
//! a cosmetic problem: it is tenant traffic encapsulated towards a leaf that will
//! drop it, with nothing to say so.
//!
//! Nothing in this file writes forwarding state. Every entry that appears got
//! there through an EVPN UPDATE and every entry that disappears did so because
//! the control plane withdrew it.

use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_vtep::OverlayDecision;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{LabRouter, VirtualLab, build_evpn_fabric, evpn_rt};
use toy_tcpip::stack::NetStackConfig;

const VNI: u32 = 5001;
const AS1: u32 = 65001;
const AS2: u32 = 65002;

const HOST_A: Ipv4Address = Ipv4Address([192, 168, 10, 11]);
const HOST_B: Ipv4Address = Ipv4Address([192, 168, 10, 22]);
const MAC_A: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x0A]);
const MAC_B: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x0B]);
const VTEP1: Ipv4Address = Ipv4Address([10, 0, 0, 1]);
const VTEP2: Ipv4Address = Ipv4Address([10, 0, 0, 2]);

fn settle(lab: &mut VirtualLab) {
    lab.run_until(250, 60_000, |_| false);
}

/// Brings the fabric up and makes both hosts speak, so each leaf has learned its
/// local MAC and advertised it.
fn converged() -> VirtualLab {
    let mut lab = build_evpn_fabric(AS1, AS2);
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
        .ping4(HOST_B, 1, 1, b"x")
        .unwrap();
    lab.send_from_host("host_a", frame);
    settle(&mut lab);
    assert_eq!(
        remote(&lab, "leaf2", MAC_A),
        Some(VTEP1),
        "the fabric never converged, so nothing after this would mean anything"
    );
    assert_eq!(remote(&lab, "leaf1", MAC_B), Some(VTEP2));
    lab
}

fn remote(lab: &VirtualLab, leaf: &str, mac: MacAddress) -> Option<Ipv4Address> {
    lab.router(leaf)?.vtep()?.lookup_remote(VNI, &mac)
}

fn evpn_routes_from_peer(lab: &VirtualLab, leaf: &str) -> usize {
    lab.router(leaf)
        .and_then(|r| r.bgp())
        .map(|b| b.evpn_adj_rib_in.total_routes())
        .unwrap_or(0)
}

// ============================================================================
// Host disappearance
// ============================================================================

#[test]
fn test_a_withdrawn_host_takes_the_remote_forwarding_entry_with_it() {
    let mut lab = converged();

    // Host A goes away. The leaf stops knowing it locally, which is what makes
    // the speaker stop originating the Type 2 route.
    lab.router_mut("leaf1")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .forget_local(VNI, &MAC_A);
    settle(&mut lab);

    assert_eq!(
        remote(&lab, "leaf2", MAC_A),
        None,
        "leaf2 still forwards host A to a VTEP that no longer claims it"
    );
    // The withdrawal reached the far Adj-RIB-In, not merely the near data plane.
    let bgp = lab.router("leaf2").unwrap().bgp().unwrap();
    assert!(
        !bgp.evpn_adj_rib_in
            .iter_paths()
            .any(|p| p.route.mac() == Some(MAC_A)),
        "the MAC/IP route survived in leaf2's Adj-RIB-In"
    );

    // Host B is untouched: a withdrawal must remove one route, not the session's.
    assert_eq!(remote(&lab, "leaf1", MAC_B), Some(VTEP2));
    // The Type 3 route is still there, so the VNI is still reachable for BUM.
    assert!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap()
            .remote_vteps
            .contains(&VTEP1)
    );
}

#[test]
fn test_traffic_for_a_withdrawn_host_is_no_longer_sent_to_the_old_vtep() {
    let mut lab = converged();
    lab.router_mut("leaf1")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .forget_local(VNI, &MAC_A);
    settle(&mut lab);

    // Leaf2 must not resolve host A to a tunnel any more. Falling back to the
    // Type 3 flood list is the honest answer for a MAC nobody advertises;
    // keeping the old unicast entry would be a black hole with no symptom.
    assert_eq!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .forward("eth0", MAC_A),
        OverlayDecision::Flood {
            vni: VNI,
            vteps: vec![VTEP1]
        },
        "leaf2 still resolves the withdrawn MAC as known unicast"
    );

    // Host B, which was never withdrawn, still resolves to its own leaf. The
    // withdrawal removed one entry, not the table.
    assert_eq!(
        lab.router("leaf1")
            .unwrap()
            .vtep()
            .unwrap()
            .forward("eth0", MAC_B),
        OverlayDecision::Unicast {
            vni: VNI,
            vtep: VTEP2
        }
    );

    // Once the session itself goes, even the flood list is empty, so nothing at
    // all is sent towards the leaf that is gone.
    lab.link_mut("leaf1spine").unwrap().set_blackhole(true);
    lab.run_until(1_000, 120_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.vtep())
            .all(|v| v.instance(VNI).is_none_or(|i| i.remote_vteps.is_empty()))
    });
    assert_eq!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .forward("eth0", MAC_A),
        OverlayDecision::Drop
    );
}

// ============================================================================
// Session failure
// ============================================================================

#[test]
fn test_a_dead_session_purges_the_evpn_state_it_taught() {
    let mut lab = converged();
    assert!(evpn_routes_from_peer(&lab, "leaf2") > 0);

    // Cut the underlay under the session. Nothing is told to withdraw anything;
    // the hold timer has to notice.
    lab.link_mut("leaf1spine").unwrap().set_blackhole(true);
    let down = lab.run_until(1_000, 120_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| !p.is_established()))
    });
    assert!(down, "the session never went down after the link was cut");

    for leaf in ["leaf1", "leaf2"] {
        assert_eq!(
            evpn_routes_from_peer(&lab, leaf),
            0,
            "{} kept EVPN routes from a peer that is gone",
            leaf
        );
        let vtep = lab.router(leaf).unwrap().vtep().unwrap();
        assert_eq!(
            vtep.remote_mac_count(),
            0,
            "{} still has remote MAC entries after the session died",
            leaf
        );
        assert!(
            vtep.instance(VNI).unwrap().remote_vteps.is_empty(),
            "{} still floods to a VTEP it can no longer reach",
            leaf
        );
        // Local state is not the peer's to take away.
        assert!(vtep.local_mac_count() > 0, "{} forgot its own host", leaf);
    }
}

#[test]
fn test_the_fabric_relearns_everything_when_the_session_comes_back() {
    let mut lab = converged();
    lab.link_mut("leaf1spine").unwrap().set_blackhole(true);
    lab.run_until(1_000, 120_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| !p.is_established()))
    });
    assert_eq!(remote(&lab, "leaf2", MAC_A), None);

    lab.link_mut("leaf1spine").unwrap().set_blackhole(false);
    let back = lab.run_until(250, 180_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.vtep())
            .all(|v| v.remote_mac_count() > 0)
    });
    assert!(
        back,
        "the overlay never came back after the link was repaired"
    );

    assert_eq!(remote(&lab, "leaf2", MAC_A), Some(VTEP1));
    assert_eq!(remote(&lab, "leaf1", MAC_B), Some(VTEP2));
    // Relearned, not left over: the session really did go down and come up again.
    assert!(
        lab.router("leaf1").unwrap().bgp().unwrap().peers()[0].establishment_count >= 2,
        "the session never re-established"
    );
}

// ============================================================================
// MAC mobility
// ============================================================================

/// Moves host A behind leaf2 by giving leaf2 a second access port in the same
/// VNI and attaching a host with host A's identity to it.
///
/// This is what a workload migration looks like from the fabric's point of view:
/// the same MAC and IP, on a different leaf, with no coordination between the
/// two leaves other than EVPN itself.
fn move_host_a_to_leaf2(lab: &mut VirtualLab) {
    lab.add_link("tenant1_moved");
    lab.add_host(
        "host_a_moved",
        "tenant1_moved",
        NetStackConfig {
            mac: MAC_A,
            ip: HOST_A,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );
    let leaf2 = lab.router_mut("leaf2").unwrap();
    leaf2.add_interface(
        "eth2",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x02]),
        Ipv4Address::new(192, 168, 10, 3),
        24,
        "tenant1_moved",
    );
    leaf2.attach_evpn_access_port(VNI, "eth2");
}

#[test]
fn test_a_host_that_moves_takes_the_forwarding_entry_with_it() {
    let mut lab = converged();
    assert_eq!(remote(&lab, "leaf2", MAC_A), Some(VTEP1));

    move_host_a_to_leaf2(&mut lab);

    // The moved host speaks. That is the only trigger: leaf2 learns MAC A on a
    // local port, sees it already claimed by VTEP1, and advertises a higher
    // mobility sequence.
    let frame = lab
        .host_mut("host_a_moved")
        .unwrap()
        .stack
        .ping4(HOST_B, 2, 2, b"moved")
        .unwrap();
    lab.send_from_host("host_a_moved", frame);
    settle(&mut lab);

    // Leaf1 followed the move: it stopped claiming host A and now points at VTEP2.
    assert_eq!(
        remote(&lab, "leaf1", MAC_A),
        Some(VTEP2),
        "leaf1 did not follow host A to its new location"
    );
    assert!(
        !lab.router("leaf1")
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap()
            .local_macs
            .contains_key(&MAC_A),
        "leaf1 still claims a host that moved away"
    );

    // Leaf2 owns it locally now and no longer treats it as remote, so there is
    // exactly one active location rather than two.
    let leaf2 = lab.router("leaf2").unwrap().vtep().unwrap();
    assert!(leaf2.instance(VNI).unwrap().local_macs.contains_key(&MAC_A));
    assert_eq!(leaf2.lookup_remote(VNI, &MAC_A), None);
}

#[test]
fn test_the_mobility_sequence_number_is_what_orders_the_two_locations() {
    let mut lab = converged();
    move_host_a_to_leaf2(&mut lab);
    let frame = lab
        .host_mut("host_a_moved")
        .unwrap()
        .stack
        .ping4(HOST_B, 3, 3, b"seq")
        .unwrap();
    lab.send_from_host("host_a_moved", frame);
    settle(&mut lab);

    // Leaf2 advertised the new location with a sequence above the old one.
    let seq = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap()
        .local_macs
        .get(&MAC_A)
        .expect("leaf2 never learned the moved host")
        .sequence;
    assert!(seq >= 1, "the moved host kept the original sequence number");

    // And that is the number leaf1 selected on.
    let path = lab
        .router("leaf1")
        .unwrap()
        .bgp()
        .unwrap()
        .evpn_loc_rib
        .iter()
        .map(|(_, p)| p)
        .find(|p| p.route.mac() == Some(MAC_A))
        .expect("leaf1 has no best path for host A");
    assert_eq!(path.route.mobility_seq, Some(seq));
    assert_eq!(path.route.next_hop, VTEP2);
}

#[test]
fn test_traffic_follows_the_host_to_its_new_leaf() {
    let mut lab = converged();
    move_host_a_to_leaf2(&mut lab);
    let frame = lab
        .host_mut("host_a_moved")
        .unwrap()
        .stack
        .ping4(HOST_B, 4, 4, b"follow")
        .unwrap();
    lab.send_from_host("host_a_moved", frame);
    settle(&mut lab);

    // Host B answers towards MAC A. Leaf2 must now bridge that locally instead of
    // encapsulating it back across the fabric to a leaf that no longer has it.
    let decision = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .forward("eth0", MAC_A);
    assert_eq!(
        decision,
        OverlayDecision::Local {
            access_interface: "eth2".to_string()
        },
        "leaf2 did not bridge to the moved host locally"
    );

    // Leaf1, meanwhile, sends anything for host A over the tunnel to VTEP2.
    assert_eq!(
        lab.router("leaf1")
            .unwrap()
            .vtep()
            .unwrap()
            .forward("eth0", MAC_A),
        OverlayDecision::Unicast {
            vni: VNI,
            vtep: VTEP2
        }
    );
}

// ============================================================================
// Guard: the fabric builder is what the tests think it is
// ============================================================================

#[test]
fn test_a_leaf_with_no_peer_learns_nothing_at_all() {
    // The negative control for every test above. Same leaf, same instance, same
    // local host - but no session, so no remote state can exist. If this ever
    // starts finding remote MACs, something is manufacturing them locally.
    let mut leaf = LabRouter::new("lonely");
    leaf.add_interface(
        "eth0",
        MacAddress([2, 0, 0, 0, 9, 0]),
        Ipv4Address::new(192, 168, 10, 1),
        24,
        "tenant",
    );
    leaf.enable_bgp(AS1, Ipv4Address::new(9, 9, 9, 9));
    leaf.enable_vtep(VTEP1, "eth0");
    leaf.add_evpn_instance(
        VNI,
        RouteDistinguisher::new(VTEP1, VNI as u16),
        &[evpn_rt(65001, VNI)],
        &[evpn_rt(65001, VNI)],
    );
    leaf.attach_evpn_access_port(VNI, "eth0");
    leaf.vtep_mut().unwrap().learn_local("eth0", MAC_A, None);

    let mut lab = VirtualLab::new();
    lab.add_link("tenant");
    lab.add_router(leaf);
    settle(&mut lab);

    let vtep = lab.router("lonely").unwrap().vtep().unwrap();
    assert_eq!(vtep.local_mac_count(), 1);
    assert_eq!(vtep.remote_mac_count(), 0);
    assert_eq!(vtep.forward("eth0", MAC_B), OverlayDecision::Drop);
}
