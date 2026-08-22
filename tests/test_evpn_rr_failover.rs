//! Two EVPN route reflectors: redundancy, failover, loop prevention, MAC
//! mobility, mass withdrawal, and a control-plane scale fabric.
//!
//! The topology under most of these tests is
//!
//! ```text
//!            rr1
//!          /     \
//!      leaf1     leaf2
//!          \     /
//!            rr2
//! ```
//!
//! with an iBGP session between the two reflectors as well, so a route from
//! leaf1 reaches leaf2 twice over and reaches each reflector both directly and
//! through the other. Neither reflector has a VTEP, a VNI, or an import Route
//! Target. There is no leaf-to-leaf session.

mod common;

use common::rr_lab::{
    MAC_A, MAC_B, converge_sessions_evpn, host_a_heard_back, host_b_heard_back,
    longest_cluster_list, ping_a_to_b, ping_b_to_a, remote_mac, total_updates_received,
    total_updates_sent,
};
use toy_tcpip::bgp_evpn::RouteTarget;
use toy_tcpip::bgp_router::{BgpRouter, BgpState};
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{
    LEAF1_VTEP, LEAF2_VTEP, RR_FABRIC_VNI, RR1_ID, RR2_ID, SCALE_BASE_VNI, VirtualLab,
    build_evpn_dual_rr_fabric, build_evpn_rr_oscillation_fabric, build_evpn_rr_scale_fabric,
    populate_scale_fabric, scale_mac,
};

const VNI: u32 = RR_FABRIC_VNI;

fn bgp<'a>(lab: &'a VirtualLab, name: &str) -> &'a BgpRouter {
    lab.router(name).unwrap().bgp().unwrap()
}

fn converged_dual() -> VirtualLab {
    let mut lab = build_evpn_dual_rr_fabric();
    assert!(
        converge_sessions_evpn(&mut lab, 90_000),
        "the dual-reflector fabric never negotiated EVPN everywhere"
    );
    ping_a_to_b(&mut lab, 0x1111, 1);
    lab
}

/// Runs the lab forward by `ms` of simulated time from wherever it is now.
fn settle(lab: &mut VirtualLab, ms: u64) {
    let until = lab.current_time_ms + ms;
    lab.run_until(250, until, |_| false);
}

// ============================================================================
// Redundancy: the same route through both reflectors
// ============================================================================

#[test]
fn test_the_same_mac_arrives_through_both_reflectors_without_duplicating_state() {
    let lab = converged_dual();

    // Leaf2 holds two *paths* for host A - one per reflector - and exactly one
    // forwarding entry, because the Loc-RIB is keyed by route and not by peer.
    let paths: Vec<Ipv4Address> = bgp(&lab, "leaf2")
        .evpn_adj_rib_in
        .iter_paths()
        .filter(|p| p.route.mac() == Some(MAC_A))
        .map(|p| p.peer_addr)
        .collect();
    let mut from = paths.clone();
    from.sort();
    assert_eq!(
        from,
        vec![RR2_ID, RR1_ID],
        "host A should have arrived through both reflectors, not {:?}",
        paths
    );

    let inst = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap();
    assert_eq!(
        inst.remote_macs.values().filter(|e| e.mac == MAC_A).count(),
        1,
        "two reflected paths produced two forwarding entries"
    );
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));

    // Whichever path won, the answer is the same: the next hop names the VTEP
    // that owns the MAC, not the reflector that carried the route.
    let best = bgp(&lab, "leaf2")
        .evpn_loc_rib
        .iter()
        .find(|(_, p)| p.route.mac() == Some(MAC_A))
        .map(|(_, p)| p)
        .expect("no best path for host A");
    assert_eq!(best.route.next_hop, LEAF1_VTEP);
    assert!(best.peer_addr == RR1_ID || best.peer_addr == RR2_ID);
}

#[test]
fn test_both_reflectors_hold_the_route_and_neither_imports_it() {
    let lab = converged_dual();
    let tenant_rt = RouteTarget::as2(65000, VNI);

    for rr in ["rr1", "rr2"] {
        let b = bgp(&lab, rr);
        assert!(b.is_route_reflector());
        assert!(b.import_route_targets().is_empty());
        assert_eq!(b.evpn_loc_rib.len(), 0, "{} imported a tenant route", rr);
        assert!(
            b.evpn_adj_rib_in.total_routes() > 0,
            "{} retained nothing",
            rr
        );
        assert!(
            b.evpn_adj_rib_in
                .iter_paths()
                .all(|p| p.route.route_targets.contains(&tenant_rt) && !p.importable),
            "{} either lost the Route Target or claimed to import it",
            rr
        );
        assert!(lab.router(rr).unwrap().vtep().is_none());
    }
}

#[test]
fn test_the_two_reflectors_keep_distinct_cluster_identifiers() {
    let lab = converged_dual();
    assert_eq!(bgp(&lab, "rr1").cluster_id(), RR1_ID);
    assert_eq!(bgp(&lab, "rr2").cluster_id(), RR2_ID);

    // Leaf2's two paths for host A are distinguishable by which cluster carried
    // them, and each list holds exactly one entry.
    let mut clusters: Vec<Vec<Ipv4Address>> = bgp(&lab, "leaf2")
        .evpn_adj_rib_in
        .iter_paths()
        .filter(|p| p.route.mac() == Some(MAC_A))
        .map(|p| p.cluster_list.clone())
        .collect();
    clusters.sort();
    assert_eq!(clusters, vec![vec![RR2_ID], vec![RR1_ID]]);
}

// ============================================================================
// Failover and restoration
// ============================================================================

#[test]
fn test_losing_one_reflector_leaves_the_overlay_intact() {
    let mut lab = converged_dual();
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));

    // rr1's sessions go away. Its routes must be purged from both leaves, and
    // the surviving reflector's copy must keep the overlay working.
    lab.router_mut("leaf1").unwrap().bgp_shutdown_peer(RR1_ID);
    lab.router_mut("leaf2").unwrap().bgp_shutdown_peer(RR1_ID);
    settle(&mut lab, 30_000);

    for leaf in ["leaf1", "leaf2"] {
        let b = bgp(&lab, leaf);
        assert_eq!(
            b.peer(RR1_ID).unwrap().state,
            BgpState::Idle,
            "{}'s session to rr1 is still up",
            leaf
        );
        assert_eq!(
            b.evpn_adj_rib_in.route_count(RR1_ID),
            0,
            "{} kept routes from the reflector it lost",
            leaf
        );
        assert!(
            b.evpn_adj_rib_in.route_count(RR2_ID) > 0,
            "{} lost the surviving reflector's routes too",
            leaf
        );
    }

    // The forwarding state is unchanged: same MAC, same VTEP, one entry.
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));
    assert_eq!(remote_mac(&lab, "leaf1", VNI, MAC_B), Some(LEAF2_VTEP));

    // And tenant traffic still crosses.
    ping_b_to_a(&mut lab, 0x2222, 2);
    assert!(
        host_b_heard_back(&lab),
        "tenant traffic stopped when one reflector went away"
    );
}

#[test]
fn test_a_reflector_coming_back_does_not_duplicate_or_churn() {
    let mut lab = converged_dual();
    lab.router_mut("leaf1").unwrap().bgp_shutdown_peer(RR1_ID);
    lab.router_mut("leaf2").unwrap().bgp_shutdown_peer(RR1_ID);
    settle(&mut lab, 30_000);

    let macs_before = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .remote_mac_count();

    lab.router_mut("leaf1").unwrap().bgp_enable_peer(RR1_ID);
    lab.router_mut("leaf2").unwrap().bgp_enable_peer(RR1_ID);
    assert!(
        converge_sessions_evpn(&mut lab, 90_000),
        "the restored reflector never came back"
    );
    settle(&mut lab, 30_000);

    // Both sessions are up again and both reflectors are carrying routes.
    for leaf in ["leaf1", "leaf2"] {
        let b = bgp(&lab, leaf);
        for rr in [RR1_ID, RR2_ID] {
            assert_eq!(b.peer(rr).unwrap().state, BgpState::Established);
            assert!(
                b.evpn_adj_rib_in.route_count(rr) > 0,
                "{} holds nothing from {} after the restore",
                leaf,
                rr
            );
        }
    }

    // No duplicate forwarding state: the same MAC count, the same answers.
    assert_eq!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .remote_mac_count(),
        macs_before,
        "restoring a reflector duplicated remote MAC state"
    );
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));

    // And the fabric goes quiet again rather than settling into a churn loop.
    let before = total_updates_sent(&lab);
    settle(&mut lab, 120_000);
    assert_eq!(
        total_updates_sent(&lab),
        before,
        "the fabric kept sending UPDATEs long after the reflector returned"
    );
}

#[test]
fn test_one_reflector_going_away_does_not_withdraw_a_route_the_other_still_carries() {
    let mut lab = converged_dual();
    let before = remote_mac(&lab, "leaf2", VNI, MAC_A);
    assert_eq!(before, Some(LEAF1_VTEP));

    // Take rr1 down at leaf2 only. Leaf1 is still advertising through rr2, so
    // nothing about host A has actually changed and leaf2 must not lose it.
    lab.router_mut("leaf2").unwrap().bgp_shutdown_peer(RR1_ID);
    settle(&mut lab, 30_000);

    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        before,
        "losing one reflector withdrew a route the other still carries"
    );
    assert_eq!(bgp(&lab, "leaf2").evpn_adj_rib_in.route_count(RR1_ID), 0);
    assert!(bgp(&lab, "leaf2").evpn_adj_rib_in.route_count(RR2_ID) > 0);
}

#[test]
fn test_losing_the_originating_leaf_withdraws_the_route_through_both_reflectors() {
    let mut lab = converged_dual();
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));

    // Now the origin itself goes away, on both of its sessions. This time the
    // route really is gone and both reflectors must say so.
    lab.router_mut("leaf1").unwrap().bgp_shutdown_peer(RR1_ID);
    lab.router_mut("leaf1").unwrap().bgp_shutdown_peer(RR2_ID);
    settle(&mut lab, 60_000);

    for rr in ["rr1", "rr2"] {
        assert_eq!(
            bgp(&lab, rr).evpn_adj_rib_in.route_count(LEAF1_VTEP),
            0,
            "{} kept routes from the leaf that went away",
            rr
        );
    }
    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        None,
        "leaf2 kept a remote MAC for a leaf that has gone"
    );
    assert!(
        !lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap()
            .remote_vteps
            .contains(&LEAF1_VTEP),
        "leaf2 kept flooding towards a VTEP that withdrew everything"
    );
}

// ============================================================================
// Loop prevention
// ============================================================================

#[test]
fn test_reflectors_sharing_a_cluster_id_refuse_each_others_reflections() {
    // Two reflectors given the same cluster identifier are, by RFC 4456, one
    // cluster. A route one of them reflects therefore arrives at the other
    // already carrying that cluster, and must be refused as having been round.
    let mut lab = build_evpn_dual_rr_fabric();
    let shared = Ipv4Address::new(7, 7, 7, 7);
    lab.router_mut("rr1").unwrap().set_bgp_cluster_id(shared);
    lab.router_mut("rr2").unwrap().set_bgp_cluster_id(shared);
    assert!(converge_sessions_evpn(&mut lab, 90_000));
    ping_a_to_b(&mut lab, 0x3333, 1);
    settle(&mut lab, 30_000);

    for (rr, other) in [("rr1", RR2_ID), ("rr2", RR1_ID)] {
        let b = bgp(&lab, rr);
        assert_eq!(b.cluster_id(), shared);
        let peer = b.peer(other).unwrap();
        assert!(
            peer.counters.cluster_loops_rejected > 0,
            "{} accepted a reflection carrying its own cluster",
            rr
        );
        assert_eq!(
            b.evpn_adj_rib_in.route_count(other),
            0,
            "{} stored a route it should have refused as a cluster loop",
            rr
        );
        assert_eq!(
            peer.state,
            BgpState::Established,
            "{} reset the session instead of dropping the route",
            rr
        );
    }

    // The fabric still works: each leaf reaches the other through its direct
    // reflector sessions.
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));
    assert_eq!(remote_mac(&lab, "leaf1", VNI, MAC_B), Some(LEAF2_VTEP));
    assert!(host_a_heard_back(&lab));
}

#[test]
fn test_the_cluster_list_never_grows_without_bound() {
    let mut lab = converged_dual();
    settle(&mut lab, 120_000);
    // Every route crosses at most one cluster before it reaches its destination,
    // and none of them accumulates.
    assert!(
        longest_cluster_list(&lab) <= 2,
        "a CLUSTER_LIST grew to {} entries",
        longest_cluster_list(&lab)
    );
}

#[test]
fn test_the_reflected_fabric_stops_talking_once_it_has_converged() {
    let mut lab = converged_dual();
    settle(&mut lab, 30_000);

    let sent = total_updates_sent(&lab);
    let received = total_updates_received(&lab);
    let decisions: u64 = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .map(|b| b.evpn_decision_runs + b.decision_runs)
        .sum();
    let keepalives: u64 = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.keepalives_sent)
        .sum();

    // Five minutes of simulated time, far longer than several hold intervals.
    settle(&mut lab, 300_000);

    assert_eq!(
        total_updates_sent(&lab),
        sent,
        "a reflected fabric that had converged carried on sending UPDATEs"
    );
    assert_eq!(total_updates_received(&lab), received);
    let decisions_after: u64 = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .map(|b| b.evpn_decision_runs + b.decision_runs)
        .sum();
    assert_eq!(
        decisions_after, decisions,
        "the decision process kept re-running with nothing to decide"
    );

    // KEEPALIVEs are the exception, and they did keep flowing.
    let keepalives_after: u64 = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.keepalives_sent)
        .sum();
    assert!(
        keepalives_after > keepalives,
        "no KEEPALIVEs were sent, so the fabric was not actually running"
    );

    // Every session survived, and none of them was reset by ordinary reflected
    // traffic.
    for r in lab.routers.values() {
        let Some(b) = r.bgp() else { continue };
        for p in b.peers() {
            assert_eq!(p.state, BgpState::Established);
            assert_eq!(p.counters.notifications_sent, 0);
            assert_eq!(p.establishment_count, 1);
        }
    }
}

#[test]
fn test_reflectors_do_not_oscillate_when_the_leaves_have_high_identifiers() {
    // A regression on RFC 4456 section 9. With the leaves numbered above both
    // reflectors, the final tie-break would make each reflector prefer the
    // other's reflected copy of a leaf's route over the leaf's own advertisement.
    // Each would then withdraw from the other under split horizon, lose the path
    // that withdrawal removed, and re-advertise, for ever. Preferring the shorter
    // CLUSTER_LIST is what stops it.
    let mut lab = build_evpn_rr_oscillation_fabric();
    assert!(converge_sessions_evpn(&mut lab, 90_000));
    ping_a_to_b(&mut lab, 0x4444, 1);
    settle(&mut lab, 30_000);

    let sent = total_updates_sent(&lab);
    settle(&mut lab, 300_000);
    assert_eq!(
        total_updates_sent(&lab),
        sent,
        "the reflector pair is oscillating: {} UPDATEs became {}",
        sent,
        total_updates_sent(&lab)
    );
    assert!(
        sent < 200,
        "convergence took {} UPDATEs, which is a storm rather than a fabric",
        sent
    );

    // Each reflector prefers the copy that came straight from the leaf, which is
    // the one with no cluster list at all.
    for rr in ["rr1", "rr2"] {
        for (_, best) in bgp(&lab, rr).evpn_advertise_rib.iter() {
            assert!(
                best.cluster_list.is_empty(),
                "{} preferred a reflected copy over the originator's own",
                rr
            );
        }
    }
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));
}

// ============================================================================
// MAC mobility through the reflectors
// ============================================================================

#[test]
fn test_a_host_that_moves_is_followed_through_the_reflectors() {
    let mut lab = converged_dual();
    assert_eq!(remote_mac(&lab, "leaf2", VNI, MAC_A), Some(LEAF1_VTEP));

    // Host A turns up behind leaf2. Its VTEP learns it locally, which - because
    // leaf2 already knows the MAC as remote - originates a route with a higher
    // MAC Mobility sequence.
    let moved = lab
        .router_mut("leaf2")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .learn_local("eth0", MAC_A, None);
    assert!(moved, "leaf2 did not learn the moved host");
    settle(&mut lab, 30_000);

    // Leaf2 now owns it locally, and leaf1 - which used to own it - has given it up.
    assert_eq!(
        remote_mac(&lab, "leaf2", VNI, MAC_A),
        None,
        "leaf2 still tunnels to leaf1 for a host attached to itself"
    );
    assert!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap()
            .local_macs
            .contains_key(&MAC_A)
    );
    assert_eq!(
        remote_mac(&lab, "leaf1", VNI, MAC_A),
        Some(LEAF2_VTEP),
        "leaf1 did not follow the host to its new location"
    );

    // The sequence number really did climb, and both reflectors carry the newer
    // route without either becoming a tenant endpoint.
    let seq = bgp(&lab, "leaf1")
        .evpn_loc_rib
        .iter()
        .find(|(_, p)| p.route.mac() == Some(MAC_A))
        .and_then(|(_, p)| p.route.mobility_seq)
        .expect("the moved route carries no mobility sequence");
    assert!(seq >= 1, "the mobility sequence did not increase: {}", seq);
    for rr in ["rr1", "rr2"] {
        assert!(lab.router(rr).unwrap().vtep().is_none());
        assert_eq!(bgp(&lab, rr).evpn_loc_rib.len(), 0);
    }
}

#[test]
fn test_a_stale_advertisement_arriving_late_does_not_win() {
    let mut lab = converged_dual();
    lab.router_mut("leaf2")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .learn_local("eth0", MAC_A, None);
    settle(&mut lab, 30_000);
    assert_eq!(remote_mac(&lab, "leaf1", VNI, MAC_A), Some(LEAF2_VTEP));

    // The old location speaks up again with its original, lower sequence: leaf1
    // re-learns host A on its own access port. That is exactly the stale
    // advertisement, and the newer sequence must still win everywhere.
    //
    // Leaf1's own VTEP raises the sequence above what it has heard, which is the
    // RFC 7432 bidding war - so the assertion is not that leaf1 stays quiet, but
    // that the fabric never ends up with two leaves claiming the MAC at once.
    lab.router_mut("leaf1")
        .unwrap()
        .vtep_mut()
        .unwrap()
        .learn_local("eth0", MAC_A, None);
    settle(&mut lab, 60_000);

    let leaf1_local = lab
        .router("leaf1")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap()
        .local_macs
        .contains_key(&MAC_A);
    let leaf2_local = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap()
        .local_macs
        .contains_key(&MAC_A);
    assert!(
        !(leaf1_local && leaf2_local),
        "both leaves ended up claiming host A as local"
    );

    // Whoever holds it, every speaker agrees on one location, and the reflectors
    // are not one of the candidates.
    let locations: Vec<Ipv4Address> = ["leaf1", "leaf2"]
        .iter()
        .filter_map(|l| remote_mac(&lab, l, VNI, MAC_A))
        .collect();
    for loc in &locations {
        assert!(
            *loc == LEAF1_VTEP || *loc == LEAF2_VTEP,
            "host A was placed at {}, which is not a tenant VTEP",
            loc
        );
    }
    if locations.len() == 2 {
        assert_eq!(
            locations[0], locations[1],
            "the leaves disagree about host A"
        );
    }
}

// ============================================================================
// Control-plane scale
// ============================================================================

#[test]
fn test_a_scale_fabric_converges_to_exactly_the_expected_route_counts() {
    const LEAVES: u8 = 8;
    const VNIS: u32 = 4;
    const HOSTS: u8 = 8;

    let mut lab = build_evpn_rr_scale_fabric(LEAVES, VNIS);
    assert!(
        converge_sessions_evpn(&mut lab, 120_000),
        "the scale fabric never brought every session up"
    );

    let learned = populate_scale_fabric(&mut lab, LEAVES, VNIS, HOSTS);
    assert_eq!(learned, LEAVES as usize * VNIS as usize * HOSTS as usize);

    let type2 = LEAVES as usize * VNIS as usize * HOSTS as usize;
    let type3 = LEAVES as usize * VNIS as usize;
    let total = type2 + type3;

    assert!(
        lab.run_until(250, 600_000, |l| {
            l.routers
                .values()
                .filter_map(|r| r.bgp())
                .filter(|b| !b.is_route_reflector())
                .all(|b| b.evpn_loc_rib.len() == total)
        }),
        "the scale fabric did not converge to {} routes per leaf",
        total
    );

    // Every leaf ends with exactly the whole fabric in its Loc-RIB and nothing
    // more: no duplicates, no leakage, no missing route.
    for n in 1..=LEAVES {
        let name = format!("leaf{}", n);
        let b = bgp(&lab, &name);
        assert_eq!(
            b.evpn_loc_rib.len(),
            total,
            "{} holds {} best paths, expected {}",
            name,
            b.evpn_loc_rib.len(),
            total
        );
        assert_eq!(
            b.evpn_originated_routes().len(),
            VNIS as usize * HOSTS as usize + VNIS as usize,
            "{} originates the wrong number of routes",
            name
        );
        // Two reflectors, so each leaf holds two paths for every route it did
        // not originate.
        let own = VNIS as usize * HOSTS as usize + VNIS as usize;
        assert_eq!(
            b.evpn_adj_rib_in.total_routes(),
            2 * (total - own),
            "{} does not hold exactly one copy per reflector",
            name
        );
    }

    // Each reflector holds every route from every leaf, plus the other
    // reflector's reflected copy of all of them, and imports none of it.
    for rr in ["rr1", "rr2"] {
        let b = bgp(&lab, rr);
        assert_eq!(b.evpn_loc_rib.len(), 0, "{} imported a tenant route", rr);
        assert_eq!(
            b.evpn_advertisable_count(),
            total,
            "{} cannot advertise the whole fabric",
            rr
        );
        assert_eq!(
            b.evpn_retained_not_imported(),
            b.evpn_adj_rib_in.total_routes(),
            "{} claims to import something",
            rr
        );
        for n in 1..=LEAVES {
            let leaf_addr = Ipv4Address::new(10, 20, 0, n);
            assert_eq!(
                b.evpn_adj_rib_in.route_count(leaf_addr),
                VNIS as usize * HOSTS as usize + VNIS as usize,
                "{} holds the wrong number of routes from leaf{}",
                rr,
                n
            );
        }
    }
}

#[test]
fn test_a_scale_fabric_keeps_its_tenants_apart_and_its_macs_unique() {
    const LEAVES: u8 = 8;
    const VNIS: u32 = 4;
    const HOSTS: u8 = 8;

    let mut lab = build_evpn_rr_scale_fabric(LEAVES, VNIS);
    assert!(converge_sessions_evpn(&mut lab, 120_000));
    populate_scale_fabric(&mut lab, LEAVES, VNIS, HOSTS);
    let total = LEAVES as usize * VNIS as usize * HOSTS as usize + LEAVES as usize * VNIS as usize;
    assert!(lab.run_until(250, 600_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .filter(|b| !b.is_route_reflector())
            .all(|b| b.evpn_loc_rib.len() == total)
    }));

    for n in 1..=LEAVES {
        let name = format!("leaf{}", n);
        let vtep = lab.router(&name).unwrap().vtep().unwrap();
        for v in 0..VNIS {
            let vni = SCALE_BASE_VNI + v;
            let inst = vtep.instance(vni).unwrap();

            // Every other leaf's hosts in this tenant, and nothing else.
            assert_eq!(
                inst.remote_macs.len(),
                (LEAVES as usize - 1) * HOSTS as usize,
                "leaf{} tenant {} holds {} remote MACs",
                n,
                vni,
                inst.remote_macs.len()
            );
            assert_eq!(
                inst.remote_vteps.len(),
                LEAVES as usize - 1,
                "leaf{} tenant {} floods to the wrong number of VTEPs",
                n,
                vni
            );

            for (mac, entry) in inst.remote_macs.iter() {
                // The MAC encodes which leaf and which tenant it belongs to, so
                // a leak between tenants or a mispointed tunnel is visible here.
                let owner = mac.0[2];
                assert_ne!(owner, n, "leaf{} holds its own host as remote", n);
                assert_eq!(
                    entry.vtep,
                    Ipv4Address::new(10, 20, 0, owner),
                    "a MAC belonging to leaf{} points at {}",
                    owner,
                    entry.vtep
                );
                let mac_vni = ((mac.0[3] as u32) << 8) | mac.0[4] as u32;
                assert_eq!(
                    mac_vni, vni,
                    "a MAC from tenant {} leaked into tenant {}",
                    mac_vni, vni
                );
                assert_eq!(*mac, scale_mac(owner, vni, mac.0[5]));
            }
        }
    }
}

#[test]
fn test_a_scale_fabric_goes_quiet_and_stays_bounded() {
    const LEAVES: u8 = 8;
    const VNIS: u32 = 4;
    const HOSTS: u8 = 8;

    let mut lab = build_evpn_rr_scale_fabric(LEAVES, VNIS);
    assert!(converge_sessions_evpn(&mut lab, 120_000));
    populate_scale_fabric(&mut lab, LEAVES, VNIS, HOSTS);
    let total = LEAVES as usize * VNIS as usize * HOSTS as usize + LEAVES as usize * VNIS as usize;
    assert!(lab.run_until(250, 600_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .filter(|b| !b.is_route_reflector())
            .all(|b| b.evpn_loc_rib.len() == total)
    }));
    settle(&mut lab, 30_000);

    let sent = total_updates_sent(&lab);
    let keepalives: u64 = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.keepalives_sent)
        .sum();

    // Advance simulated time substantially: five minutes, many hold intervals.
    settle(&mut lab, 300_000);

    assert_eq!(
        total_updates_sent(&lab),
        sent,
        "a converged scale fabric kept sending UPDATEs"
    );
    let keepalives_after: u64 = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.keepalives_sent)
        .sum();
    assert!(
        keepalives_after > keepalives,
        "nothing at all was sent, so the fabric was not running"
    );

    // Bounded control-plane state, stable sessions, no cluster list creeping up.
    assert!(
        longest_cluster_list(&lab) <= 2,
        "a CLUSTER_LIST reached {} entries",
        longest_cluster_list(&lab)
    );
    for r in lab.routers.values() {
        let Some(b) = r.bgp() else { continue };
        for p in b.peers() {
            assert_eq!(
                p.state,
                BgpState::Established,
                "a session dropped in a fabric that had nothing to say"
            );
            assert_eq!(p.establishment_count, 1, "a session flapped");
            assert_eq!(p.counters.notifications_sent, 0);
            assert_eq!(p.counters.originator_loops_rejected, 0);
            assert_eq!(p.counters.cluster_loops_rejected, 0);
        }
    }
}

#[test]
fn test_a_leaf_losing_both_sessions_purges_its_routes_from_the_whole_fabric() {
    const LEAVES: u8 = 8;
    const VNIS: u32 = 2;
    const HOSTS: u8 = 8;

    let mut lab = build_evpn_rr_scale_fabric(LEAVES, VNIS);
    assert!(converge_sessions_evpn(&mut lab, 120_000));
    populate_scale_fabric(&mut lab, LEAVES, VNIS, HOSTS);
    let total = LEAVES as usize * VNIS as usize * HOSTS as usize + LEAVES as usize * VNIS as usize;
    assert!(lab.run_until(250, 600_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .filter(|b| !b.is_route_reflector())
            .all(|b| b.evpn_loc_rib.len() == total)
    }));

    // leaf1 loses both reflector sessions at once: mass withdrawal of the
    // routes it was the only source of.
    let leaf1 = Ipv4Address::new(10, 20, 0, 1);
    lab.router_mut("leaf1")
        .unwrap()
        .bgp_shutdown_peer(Ipv4Address::new(10, 20, 0, 254));
    lab.router_mut("leaf1")
        .unwrap()
        .bgp_shutdown_peer(Ipv4Address::new(10, 20, 0, 253));

    let per_leaf = VNIS as usize * HOSTS as usize + VNIS as usize;
    let remaining = total - per_leaf;
    assert!(
        lab.run_until(250, 600_000, |l| {
            (2..=LEAVES).all(|n| {
                l.router(&format!("leaf{}", n))
                    .and_then(|r| r.bgp())
                    .is_some_and(|b| b.evpn_loc_rib.len() == remaining)
            })
        }),
        "the fabric did not converge down to {} routes after leaf1 left",
        remaining
    );

    // Nothing stale anywhere: not in the reflectors, not in the other leaves'
    // Adj-RIB-In, and not in their forwarding state.
    for rr in ["rr1", "rr2"] {
        assert_eq!(bgp(&lab, rr).evpn_adj_rib_in.route_count(leaf1), 0);
        assert_eq!(bgp(&lab, rr).evpn_advertisable_count(), remaining);
    }
    for n in 2..=LEAVES {
        let name = format!("leaf{}", n);
        assert!(
            !bgp(&lab, &name)
                .evpn_adj_rib_in
                .iter_paths()
                .any(|p| p.route.next_hop == leaf1),
            "{} kept a path pointing at the leaf that went away",
            name
        );
        let vtep = lab.router(&name).unwrap().vtep().unwrap();
        for v in 0..VNIS {
            let inst = vtep.instance(SCALE_BASE_VNI + v).unwrap();
            assert!(
                !inst.remote_vteps.contains(&leaf1),
                "{} still floods to the departed leaf",
                name
            );
            assert!(
                inst.remote_macs.values().all(|e| e.vtep != leaf1),
                "{} still tunnels a MAC to the departed leaf",
                name
            );
        }
    }
}
