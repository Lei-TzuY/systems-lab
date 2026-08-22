//! MP-BGP EVPN through a route reflector that owns no part of the tenant.
//!
//! The bar every test here has to clear is the one this phase exists for: `rr1`
//! has no VTEP, no VNI, no EVPN instance and no import Route Target, and the two
//! leaves have no BGP session to each other. Every remote MAC either leaf ends up
//! with must therefore have been retained and reflected by a router that could
//! not itself have used it, and imported at the far end by Route Target.
//!
//! No test writes a remote MAC, a remote VTEP or a tunnel destination into a leaf.

mod common;

use common::rr_lab::{
    HOST_A, HOST_B, MAC_A, MAC_B, captured_vxlan, converge_sessions_evpn, host_a_heard_back,
    ping_a_to_b, ping_b_to_a, remote_mac,
};
use toy_tcpip::bgp::BgpOrigin;
use toy_tcpip::bgp_caps::AfiSafi;
use toy_tcpip::bgp_evpn::RouteTarget;
use toy_tcpip::bgp_router::{BgpPeerMode, BgpPeerRole, BgpRouter};
use toy_tcpip::evpn::EvpnNlri;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{
    LEAF1_VTEP, LEAF2_VTEP, RR_FABRIC_AS, RR_FABRIC_VNI, RR1_ID, VirtualLab, build_evpn_rr_fabric,
};

const VNI: u32 = RR_FABRIC_VNI;
const LEAF1_ID: Ipv4Address = Ipv4Address([1, 1, 1, 1]);
const LEAF2_ID: Ipv4Address = Ipv4Address([3, 3, 3, 3]);

/// Brings the fabric up and gets both hosts talking, which is the only way either
/// leaf learns a local MAC and so the only way anything is advertised at all.
fn converged() -> VirtualLab {
    let mut lab = build_evpn_rr_fabric();
    assert!(
        converge_sessions_evpn(&mut lab, 60_000),
        "the fabric never negotiated EVPN on every session"
    );
    ping_a_to_b(&mut lab, 0x1234, 1);
    lab
}

// ============================================================================
// The reflector is configured as one, and as nothing else
// ============================================================================

#[test]
fn test_the_reflector_owns_no_tenant_and_still_peers_with_both_leaves() {
    let lab = converged();
    let rr = lab.router("rr1").unwrap();

    // Nothing about the tenant is configured on it.
    assert!(rr.vtep().is_none(), "the route reflector was given a VTEP");
    let bgp = rr.bgp().unwrap();
    assert!(
        bgp.import_route_targets().is_empty(),
        "the route reflector imports a tenant Route Target: {:?}",
        bgp.import_route_targets()
    );

    // But it is a reflector, and both leaves are its clients.
    assert!(bgp.is_route_reflector());
    assert_eq!(
        bgp.cluster_id(),
        RR1_ID,
        "the cluster ID should default to the router ID"
    );
    let mut clients = bgp.route_reflector_clients();
    clients.sort();
    assert_eq!(clients, vec![LEAF1_VTEP, LEAF2_VTEP]);
    for leaf in [LEAF1_VTEP, LEAF2_VTEP] {
        assert_eq!(bgp.peer_role(leaf), Some(BgpPeerRole::RouteReflectorClient));
        assert!(bgp.peer(leaf).unwrap().carries_evpn());
    }

    // And the leaves peer only with it.
    for leaf in ["leaf1", "leaf2"] {
        let peers: Vec<Ipv4Address> = lab
            .router(leaf)
            .unwrap()
            .bgp()
            .unwrap()
            .peers()
            .iter()
            .map(|p| p.addr)
            .collect();
        assert_eq!(
            peers,
            vec![RR1_ID],
            "{} has a BGP session it should not have",
            leaf
        );
        assert!(
            !lab.router(leaf)
                .unwrap()
                .bgp()
                .unwrap()
                .is_route_reflector(),
            "{} was configured as a reflector",
            leaf
        );
    }
}

#[test]
fn test_the_reflector_retains_the_tenant_route_without_importing_it() {
    let lab = converged();
    let bgp = lab.router("rr1").unwrap().bgp().unwrap();

    // It received the routes and kept them: a Type 2 and a Type 3 per leaf.
    let received = bgp.evpn_adj_rib_in.total_routes();
    assert_eq!(
        received, 4,
        "the reflector holds {} EVPN route(s); both leaves should have sent one \
         Type 2 and one Type 3",
        received
    );
    assert!(
        bgp.retains_all_route_targets(),
        "a route reflector must retain routes whose Route Targets it does not import"
    );

    // ...and imported none of them, because it asked for no Route Target.
    let (adj_in, loc, originated) = bgp.evpn_route_counts();
    assert_eq!(adj_in, received);
    assert_eq!(
        loc, 0,
        "the reflector imported a tenant route into its Loc-RIB"
    );
    assert_eq!(originated, 0, "the reflector originated an EVPN route");
    assert_eq!(
        bgp.evpn_retained_not_imported(),
        received,
        "every retained route should be one the reflector cannot use"
    );
    // All four are still eligible to be passed on, which is the distinction the
    // whole design turns on.
    assert_eq!(bgp.evpn_advertisable_count(), received);

    // Every stored path knows it is not importable here, and still carries the
    // tenant Route Target the reflector was never configured with.
    let tenant_rt = RouteTarget::as2(RR_FABRIC_AS as u16, VNI);
    for path in bgp.evpn_adj_rib_in.iter_paths() {
        assert!(
            !path.importable,
            "a path claimed to be importable on a reflector with no Route Targets"
        );
        assert!(
            path.route.route_targets.contains(&tenant_rt),
            "a retained route lost its Route Target: {:?}",
            path.route.route_targets
        );
        assert!(
            path.from_client,
            "a route from a client was not recorded as such"
        );
    }
}

// ============================================================================
// The acceptance chain
// ============================================================================

#[test]
fn test_a_mac_reaches_the_far_leaf_through_a_reflector_with_no_vni() {
    let lab = converged();

    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        Some(LEAF1_VTEP),
        "leaf2 never learned host A through the reflector"
    );
    assert_eq!(
        remote_mac(&lab, "leaf1", VNI, MAC_B),
        Some(LEAF2_VTEP),
        "leaf1 never learned host B through the reflector"
    );

    // The reflector itself is in nobody's forwarding state.
    for leaf in ["leaf1", "leaf2"] {
        let vtep = lab.router(leaf).unwrap().vtep().unwrap();
        let inst = vtep.instance(VNI).unwrap();
        assert!(
            !inst.remote_vteps.contains(&RR1_ID),
            "{} put the route reflector in its flood list",
            leaf
        );
        for entry in inst.remote_macs.values() {
            assert_ne!(
                entry.vtep, RR1_ID,
                "{} points a tenant MAC at the route reflector",
                leaf
            );
        }
    }
}

#[test]
fn test_the_reflected_route_carries_the_originator_and_the_cluster() {
    let lab = converged();

    // What leaf2 holds for host A arrived from the reflector, but names leaf1 as
    // the speaker that originated it and rr1's cluster as the one it crossed.
    let bgp = lab.router("leaf2").unwrap().bgp().unwrap();
    let path = bgp
        .evpn_adj_rib_in
        .iter_paths()
        .find(|p| p.route.mac() == Some(MAC_A))
        .expect("leaf2 holds no path for host A");

    assert_eq!(
        path.peer_addr, RR1_ID,
        "the path did not arrive from the reflector"
    );
    assert_eq!(
        path.originator_id,
        Some(LEAF1_ID),
        "ORIGINATOR_ID should be leaf1's BGP identifier, not the reflector's"
    );
    assert_eq!(
        path.cluster_list,
        vec![RR1_ID],
        "CLUSTER_LIST should hold exactly the one cluster the route crossed"
    );
    // The next hop was not rewritten: it still names the VTEP that owns the MAC.
    assert_eq!(path.route.next_hop, LEAF1_VTEP);

    // And the reflector counted the reflections it performed.
    let rr = lab.router("rr1").unwrap().bgp().unwrap();
    for peer in rr.peers() {
        assert!(
            peer.counters.routes_reflected > 0,
            "the reflector never counted a reflection towards {}",
            peer.addr
        );
    }
}

#[test]
fn test_a_leaf_originated_route_carries_no_reflection_metadata() {
    let lab = converged();

    // Leaf1 originates host A itself, so what it sends the reflector must be a
    // plain advertisement. Metadata attached at the origin would make the route
    // look as though it had already been round a cluster.
    let bgp = lab.router("leaf1").unwrap().bgp().unwrap();
    let key = bgp
        .evpn_adj_rib_out
        .keys(RR1_ID)
        .into_iter()
        .find(|k| k.mac() == Some(MAC_A))
        .expect("leaf1 advertised no route for host A");
    let advert = bgp.evpn_adj_rib_out.get(RR1_ID, &key).unwrap();
    assert!(
        !advert.is_reflected(),
        "leaf1 attached reflection metadata to its own route"
    );
    assert_eq!(advert.originator_id, None);
    assert!(advert.cluster_list.is_empty());
    assert_eq!(
        bgp.peers()[0].counters.routes_reflected,
        0,
        "a leaf counted a reflection it never performed"
    );
}

#[test]
fn test_tenant_traffic_crosses_vxlan_between_leaves_that_never_peered() {
    let mut lab = build_evpn_rr_fabric();
    assert!(converge_sessions_evpn(&mut lab, 60_000));
    lab.enable_pcap("leaf2rr1");
    ping_a_to_b(&mut lab, 0x1234, 1);
    ping_b_to_a(&mut lab, 0x4321, 7);

    assert!(
        host_a_heard_back(&lab),
        "host A never got a reply, so nothing crossed the overlay"
    );

    let pcap = lab
        .export_pcap("leaf2rr1")
        .expect("no capture on the leaf2 uplink");
    let vxlan = captured_vxlan(&pcap);
    assert!(
        !vxlan.is_empty(),
        "no VXLAN traffic crossed the leaf2 uplink"
    );

    // A tenant frame from host B to host A, encapsulated in the tenant VNI and
    // addressed to leaf1's VTEP, on a wire whose only BGP peer is the reflector.
    let carried: Vec<(Ipv4Address, u32, toy_tcpip::ethernet::MacAddress)> = vxlan
        .iter()
        .filter_map(|(_, dst, vni, inner)| {
            toy_tcpip::ethernet::EthernetFrame::parse(inner)
                .ok()
                .map(|e| (*dst, *vni, e.dst_mac))
        })
        .collect();
    assert!(
        carried
            .iter()
            .any(|(dst, vni, inner_dst)| *dst == LEAF1_VTEP && *vni == VNI && *inner_dst == MAC_A),
        "no tenant frame for host A was encapsulated towards leaf1: {:?}",
        carried
    );
    assert!(
        carried.iter().all(|(dst, _, _)| *dst != RR1_ID),
        "tenant traffic was encapsulated towards the route reflector"
    );
}

// ============================================================================
// Type 3 inclusive multicast through the reflector
// ============================================================================

#[test]
fn test_the_flood_list_is_built_from_type_3_routes_learned_through_the_reflector() {
    let mut lab = build_evpn_rr_fabric();
    assert!(converge_sessions_evpn(&mut lab, 60_000));
    // No host has spoken, so the only EVPN routes in the fabric are the Type 3
    // routes each leaf originates for its own instance.
    lab.run_until(250, 30_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.vtep())
            .all(|v| v.instance(VNI).is_some_and(|i| !i.remote_vteps.is_empty()))
    });

    for (leaf, expected) in [("leaf1", LEAF2_VTEP), ("leaf2", LEAF1_VTEP)] {
        let inst = lab
            .router(leaf)
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap();
        assert_eq!(
            inst.remote_vteps.iter().copied().collect::<Vec<_>>(),
            vec![expected],
            "{}'s flood list should hold exactly the far leaf",
            leaf
        );
        assert!(
            inst.remote_macs.is_empty(),
            "{} learned a remote MAC before any host spoke",
            leaf
        );
    }

    // The Type 3 routes really did travel through the reflector, which holds them
    // without belonging to the broadcast domain they describe.
    let rr = lab.router("rr1").unwrap().bgp().unwrap();
    let imets = rr
        .evpn_adj_rib_in
        .iter_paths()
        .filter(|p| matches!(p.route.nlri, EvpnNlri::InclusiveMulticast(_)))
        .count();
    assert_eq!(
        imets, 2,
        "the reflector should hold one Type 3 route per leaf"
    );
    assert_eq!(rr.evpn_loc_rib.len(), 0);
}

#[test]
fn test_flooding_reaches_only_real_tenant_vteps() {
    let mut lab = build_evpn_rr_fabric();
    assert!(converge_sessions_evpn(&mut lab, 60_000));
    lab.run_until(250, 30_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.vtep())
            .all(|v| v.instance(VNI).is_some_and(|i| !i.remote_vteps.is_empty()))
    });
    lab.enable_pcap("leaf1rr1");

    // Host A pings host B before either leaf knows anything about the other's
    // MACs. The ARP that starts it is a broadcast, and the only thing that can
    // carry it is ingress replication driven by the Type 3 routes from rr1.
    ping_a_to_b(&mut lab, 0x2222, 1);

    // The link carries both directions, so the flood set is what leaf1 *sent*:
    // packets whose outer source is leaf1's own VTEP address.
    let pcap = lab.export_pcap("leaf1rr1").expect("no capture");
    let mut destinations: Vec<Ipv4Address> = Vec::new();
    for (src, dst, _, _) in captured_vxlan(&pcap) {
        if src == LEAF1_VTEP && !destinations.contains(&dst) {
            destinations.push(dst);
        }
    }
    assert!(
        !destinations.is_empty(),
        "leaf1 replicated the tenant broadcast nowhere"
    );
    assert!(
        !destinations.contains(&RR1_ID),
        "leaf1 replicated tenant broadcast to the route reflector: {:?}",
        destinations
    );
    assert_eq!(
        destinations,
        vec![LEAF2_VTEP],
        "the flood set should be exactly the one real tenant VTEP"
    );
    assert!(
        host_a_heard_back(&lab),
        "the flooded ARP never produced an answer"
    );
}

// ============================================================================
// The propagation rules, on the EVPN family
// ============================================================================

/// Brings the single-reflector fabric up with each leaf given the role named,
/// then lets a host speak so there is something to reflect.
fn fabric_with_roles(leaf1_client: bool, leaf2_client: bool) -> VirtualLab {
    let mut lab = build_evpn_rr_fabric();
    {
        let rr = lab.router_mut("rr1").unwrap();
        rr.set_bgp_route_reflector_client(LEAF1_VTEP, leaf1_client);
        rr.set_bgp_route_reflector_client(LEAF2_VTEP, leaf2_client);
    }
    assert!(converge_sessions_evpn(&mut lab, 60_000));
    ping_a_to_b(&mut lab, 0x5555, 1);
    lab
}

#[test]
fn test_an_evpn_route_from_a_non_client_never_reaches_another_non_client() {
    // Both leaves demoted. The sessions are identical to the working fabric in
    // every other way - same AS, same capability, same Route Targets - so the
    // only thing that can stop leaf1's MAC reaching leaf2 is the propagation
    // rule, which is the point.
    let lab = fabric_with_roles(false, false);

    let rr = lab.router("rr1").unwrap().bgp().unwrap();
    assert!(
        !rr.is_route_reflector(),
        "a speaker with no clients is not a reflector"
    );
    for leaf in [LEAF1_VTEP, LEAF2_VTEP] {
        assert_eq!(
            rr.evpn_adj_rib_out.route_count(leaf),
            0,
            "an internally learned EVPN route was advertised to a non-client"
        );
    }
    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        None,
        "host A crossed a speaker that was not a reflector"
    );
    assert_eq!(remote_mac(&lab, "leaf1", VNI, MAC_B), None);
}

#[test]
fn test_an_evpn_route_from_a_client_reaches_a_non_client() {
    // leaf1 is a client, leaf2 is not. A route from a client may go anywhere, so
    // host A must still reach leaf2 - and because leaf2 is a client of nobody,
    // its own route may go only to clients, which leaf1 is.
    let lab = fabric_with_roles(true, false);

    let rr = lab.router("rr1").unwrap().bgp().unwrap();
    assert!(rr.is_route_reflector());
    assert_eq!(
        rr.peer_role(LEAF1_VTEP),
        Some(BgpPeerRole::RouteReflectorClient)
    );
    assert_eq!(rr.peer_role(LEAF2_VTEP), Some(BgpPeerRole::Normal));

    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        Some(LEAF1_VTEP),
        "client -> RR -> non-client was not reflected on the EVPN family"
    );
    assert_eq!(
        remote_mac(&lab, "leaf1", VNI, MAC_B),
        Some(LEAF2_VTEP),
        "non-client -> RR -> client was not reflected on the EVPN family"
    );

    // Both directions carry the reflection metadata, whichever end was the client.
    for (leaf, mac, originator) in [("leaf2", MAC_A, LEAF1_ID), ("leaf1", MAC_B, LEAF2_ID)] {
        let path = lab
            .router(leaf)
            .unwrap()
            .bgp()
            .unwrap()
            .evpn_adj_rib_in
            .iter_paths()
            .find(|p| p.route.mac() == Some(mac))
            .unwrap_or_else(|| panic!("{} has no path for the far host", leaf))
            .clone();
        assert_eq!(path.originator_id, Some(originator));
        assert_eq!(path.cluster_list, vec![RR1_ID]);
    }
}

#[test]
fn test_demoting_both_leaves_stops_a_working_evpn_fabric_and_promoting_restores_it() {
    // The same fabric, reconfigured while it is running. What was reflected must
    // be withdrawn, the overlay must forget it, and putting the roles back must
    // bring it all the way to tenant forwarding again.
    let mut lab = converged();
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));

    {
        let rr = lab.router_mut("rr1").unwrap();
        rr.set_bgp_route_reflector_client(LEAF1_VTEP, false);
        rr.set_bgp_route_reflector_client(LEAF2_VTEP, false);
    }
    lab.run_until(250, lab.current_time_ms + 60_000, |_| false);

    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        None,
        "demoting both clients did not withdraw what had been reflected"
    );
    assert!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap()
            .remote_vteps
            .is_empty(),
        "the flood list survived the reflector being switched off"
    );

    {
        let rr = lab.router_mut("rr1").unwrap();
        rr.set_bgp_route_reflector_client(LEAF1_VTEP, true);
        rr.set_bgp_route_reflector_client(LEAF2_VTEP, true);
    }
    lab.run_until(250, lab.current_time_ms + 60_000, |_| false);

    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        Some(LEAF1_VTEP),
        "promoting the clients back did not restore the overlay"
    );
    assert_eq!(remote_mac(&lab, "leaf1", VNI, MAC_B), Some(LEAF2_VTEP));
}

// ============================================================================
// Attribute fidelity across reflection
// ============================================================================

#[test]
fn test_reflection_preserves_every_evpn_attribute() {
    let lab = converged();

    // Compare what leaf1 originated against what leaf2 received through rr1.
    let origin = lab
        .router("leaf1")
        .unwrap()
        .bgp()
        .unwrap()
        .evpn_originated_routes()
        .into_iter()
        .find(|r| r.mac() == Some(MAC_A))
        .cloned()
        .expect("leaf1 originated nothing for host A");

    let received = lab
        .router("leaf2")
        .unwrap()
        .bgp()
        .unwrap()
        .evpn_adj_rib_in
        .iter_paths()
        .find(|p| p.route.mac() == Some(MAC_A))
        .expect("leaf2 received nothing for host A")
        .clone();

    assert_eq!(received.route.key(), origin.key(), "the route key changed");
    assert_eq!(
        received.route.key().rd(),
        origin.key().rd(),
        "Route Distinguisher changed"
    );
    assert_eq!(
        received.route.route_targets, origin.route_targets,
        "Route Targets changed"
    );
    assert_eq!(received.route.vni(), origin.vni(), "VNI changed");
    assert_eq!(received.route.mac(), origin.mac(), "MAC changed");
    assert_eq!(
        received.route.host_ip(),
        origin.host_ip(),
        "host IP changed"
    );
    assert_eq!(
        received.route.mobility_seq, origin.mobility_seq,
        "MAC Mobility sequence changed"
    );
    assert_eq!(
        received.route.next_hop, LEAF1_VTEP,
        "the MP_REACH next hop was rewritten by the reflector"
    );
    // Inside one AS the AS_PATH does not grow, and a reflector must not touch it.
    assert!(
        received.as_path.is_empty(),
        "the reflector prepended to AS_PATH: [{}]",
        received.as_path
    );
    assert_eq!(received.local_pref, 100, "LOCAL_PREF was not preserved");
    assert_eq!(received.origin, BgpOrigin::Igp, "ORIGIN was not preserved");

    // The reflection metadata is additional, not a replacement.
    assert_eq!(received.originator_id, Some(LEAF1_ID));
    assert_eq!(received.cluster_list, vec![RR1_ID]);

    // And the whole thing round-trips: leaf2 programmed it into the data plane.
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));
}

#[test]
fn test_the_host_ip_survives_reflection_so_the_far_leaf_can_answer_arp() {
    let lab = converged();
    let inst = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap();
    let entry = inst
        .remote_macs
        .get(&MAC_A)
        .expect("leaf2 has no remote entry for host A");
    assert_eq!(
        entry.ip,
        Some(HOST_A),
        "the Type 2 route lost its host IP crossing the reflector"
    );
    assert_eq!(entry.vtep, LEAF1_VTEP);

    // The reverse direction too, so this is not an artefact of one leaf.
    let inst1 = lab
        .router("leaf1")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap();
    assert_eq!(
        inst1.remote_macs.get(&MAC_B).and_then(|e| e.ip),
        Some(HOST_B)
    );
}

// ============================================================================
// Capability gating still applies to reflected routes
// ============================================================================

#[test]
fn test_a_client_without_the_evpn_capability_is_never_sent_a_reflected_route() {
    let mut lab = build_evpn_rr_fabric();
    // Replace leaf2's speaker with one that never offers L2VPN EVPN, so its
    // session with the reflector negotiates IPv4 unicast only. It is still a
    // route reflector client, so the RFC 4456 rules alone would send it
    // everything the reflector holds.
    {
        let leaf2 = lab.router_mut("leaf2").unwrap();
        let bgp = leaf2.bgp_mut().unwrap();
        *bgp = BgpRouter::new(RR_FABRIC_AS, LEAF2_ID);
        bgp.set_hold_time(9);
        bgp.add_peer(RR1_ID, RR_FABRIC_AS, LEAF2_VTEP, BgpPeerMode::Active);
    }

    lab.run_until(250, 60_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.is_established()))
    });
    ping_a_to_b(&mut lab, 7, 1);

    let leaf2_bgp = lab.router("leaf2").unwrap().bgp().unwrap();
    let peer = &leaf2_bgp.peers()[0];
    assert!(
        peer.is_established(),
        "the IPv4-only session did not come up"
    );
    assert!(!peer.negotiated.supports(AfiSafi::L2VPN_EVPN));
    assert_eq!(
        leaf2_bgp.evpn_adj_rib_in.total_routes(),
        0,
        "a peer that never negotiated EVPN was sent EVPN NLRI"
    );

    // The reflector still holds leaf1's routes; it simply has nowhere to send them.
    let rr = lab.router("rr1").unwrap().bgp().unwrap();
    assert!(rr.evpn_adj_rib_in.total_routes() > 0);
    assert_eq!(
        rr.evpn_adj_rib_out.route_count(LEAF2_VTEP),
        0,
        "the reflector recorded advertising EVPN to an IPv4-only peer"
    );
    assert_eq!(
        rr.peer(LEAF2_VTEP).unwrap().counters.evpn_advertised,
        0,
        "the reflector counted an EVPN advertisement to an IPv4-only peer"
    );
}

// ============================================================================
// PCAP: the whole chain recovered from captured bytes
// ============================================================================

#[test]
fn test_the_whole_reflected_chain_is_recoverable_from_the_capture() {
    use common::rr_lab::{bgp_stream, captured_vxlan, read_capture};
    use toy_tcpip::bgp::{BGP_PORT, BgpPdu};
    use toy_tcpip::bgp_caps::BgpCapability;
    use toy_tcpip::ethernet::EtherType;
    use toy_tcpip::ipv4::IpProtocol;

    let mut lab = build_evpn_rr_fabric();
    // Capture both uplinks: leaf1 -> rr1 carries the original advertisement,
    // rr1 -> leaf2 carries the reflection of it.
    lab.enable_pcap("leaf1rr1");
    lab.enable_pcap("leaf2rr1");
    assert!(converge_sessions_evpn(&mut lab, 60_000));
    ping_a_to_b(&mut lab, 0x9999, 1);
    ping_b_to_a(&mut lab, 0x9998, 1);

    let up = lab.export_pcap("leaf1rr1").expect("no leaf1 capture");
    let down = lab.export_pcap("leaf2rr1").expect("no leaf2 capture");
    let up_pkts = read_capture(&up);
    let down_pkts = read_capture(&down);

    // ---- the session itself, on TCP port 179 --------------------------------
    assert!(
        up_pkts.iter().any(|p| {
            p.protocol == Some(IpProtocol::Tcp)
                && toy_tcpip::tcp::TcpSegment::parse(p.src, p.dst, &p.payload, false)
                    .is_ok_and(|s| s.dst_port == BGP_PORT || s.src_port == BGP_PORT)
        }),
        "no BGP traffic on port 179 in the capture"
    );

    // ---- leaf1 -> rr1: OPEN with the EVPN capability, KEEPALIVE, MP_REACH ----
    let from_leaf1 = bgp_stream(&up_pkts, LEAF1_VTEP, RR1_ID);
    let open = from_leaf1
        .iter()
        .find_map(|m| match m {
            BgpPdu::Open(o) => Some(o),
            _ => None,
        })
        .expect("leaf1 sent no OPEN");
    let caps = open
        .capabilities()
        .expect("leaf1's OPEN had bad capabilities");
    assert!(
        caps.capabilities.iter().any(|c| matches!(
            c,
            BgpCapability::MultiProtocol(f) if *f == AfiSafi::L2VPN_EVPN
        )),
        "leaf1's OPEN did not advertise AFI 25 / SAFI 70"
    );
    assert!(
        from_leaf1.iter().any(|m| matches!(m, BgpPdu::Keepalive)),
        "no KEEPALIVE from leaf1"
    );

    let advertised: Vec<&BgpPdu> = from_leaf1
        .iter()
        .filter(|m| matches!(m, BgpPdu::Update(u) if u.mp_reach().is_some()))
        .collect();
    assert!(
        !advertised.is_empty(),
        "leaf1 sent no MP_REACH EVPN advertisement"
    );
    let originated_mac_a = advertised.iter().any(|m| {
        let BgpPdu::Update(u) = m else { return false };
        let Some(mp) = u.mp_reach() else { return false };
        mp.family() == AfiSafi::L2VPN_EVPN
            && toy_tcpip::bgp_evpn::decode_evpn_nlri_list(&mp.nlri)
                .map(|list| {
                    list.iter()
                        .any(|n| matches!(n, EvpnNlri::MacIpAdv(m2) if m2.mac == MAC_A))
                })
                .unwrap_or(false)
    });
    assert!(
        originated_mac_a,
        "the captured bytes contain no Type 2 route for host A from leaf1"
    );

    // ---- rr1 -> leaf2: the reflection, with ORIGINATOR_ID and CLUSTER_LIST --
    let reflected = bgp_stream(&down_pkts, RR1_ID, LEAF2_VTEP);
    let mut saw_reflected_mac_a = false;
    for m in &reflected {
        let BgpPdu::Update(u) = m else { continue };
        let Some(attrs) = u.attributes.as_ref() else {
            continue;
        };
        let Some(mp) = attrs.mp_reach.as_ref() else {
            continue;
        };
        if mp.family() != AfiSafi::L2VPN_EVPN {
            continue;
        }
        let Ok(list) = toy_tcpip::bgp_evpn::decode_evpn_nlri_list(&mp.nlri) else {
            continue;
        };
        if !list
            .iter()
            .any(|n| matches!(n, EvpnNlri::MacIpAdv(m2) if m2.mac == MAC_A))
        {
            continue;
        }
        saw_reflected_mac_a = true;

        // Everything the reflection is required to say, read straight off the wire.
        assert_eq!(
            attrs.originator_id,
            Some(LEAF1_ID),
            "the reflected UPDATE names the wrong originator"
        );
        assert_eq!(
            attrs.cluster_list,
            vec![RR1_ID],
            "the reflected UPDATE carries the wrong cluster list"
        );
        assert_eq!(
            mp.ipv4_next_hop(),
            Some(LEAF1_VTEP),
            "the reflector rewrote the next hop"
        );
        let rts = toy_tcpip::bgp_evpn::route_targets_from_communities(&attrs.ext_communities);
        assert!(
            rts.contains(&RouteTarget::as2(RR_FABRIC_AS as u16, VNI)),
            "the reflected UPDATE lost the tenant Route Target: {:?}",
            rts
        );
    }
    assert!(
        saw_reflected_mac_a,
        "no reflected MP_REACH for host A was recoverable from the captured TCP bytes"
    );

    // ---- the tenant data plane: ARP, then VXLAN on UDP 4789 -----------------
    let vxlan = captured_vxlan(&down);
    assert!(!vxlan.is_empty(), "no VXLAN packets in the leaf2 capture");
    let mut saw_arp = false;
    let mut saw_unicast_to_leaf1 = false;
    for (_, dst, vni, inner) in &vxlan {
        assert_eq!(*vni, VNI, "a packet was encapsulated in the wrong VNI");
        let Ok(eth) = toy_tcpip::ethernet::EthernetFrame::parse(inner) else {
            continue;
        };
        if eth.ethertype == EtherType::Arp {
            saw_arp = true;
        }
        if *dst == LEAF1_VTEP && eth.dst_mac == MAC_A {
            saw_unicast_to_leaf1 = true;
        }
        assert_ne!(
            *dst, RR1_ID,
            "tenant traffic was tunnelled to the reflector"
        );
    }
    assert!(saw_arp, "no tenant ARP crossed the overlay");
    assert!(
        saw_unicast_to_leaf1,
        "no tenant unicast was sent to leaf1's VTEP"
    );

    // ---- a withdrawal, once the host goes away -----------------------------
    lab.router_mut("leaf2")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .forget_local(VNI, &MAC_B);
    lab.run_until(250, 30_000, |_| false);
    let after = lab.export_pcap("leaf2rr1").expect("no capture");
    let withdrawals = bgp_stream(&read_capture(&after), LEAF2_VTEP, RR1_ID)
        .into_iter()
        .filter(|m| matches!(m, BgpPdu::Update(u) if u.mp_unreach().is_some()))
        .count();
    assert!(
        withdrawals > 0,
        "no MP_UNREACH withdrawal was recoverable from the capture"
    );
    assert_eq!(
        remote_mac(&lab, "leaf1", VNI, MAC_B),
        None,
        "leaf1 kept a MAC that was withdrawn through the reflector"
    );
}
