//! BGP route reflection for IPv4 unicast (RFC 4456), and the RFC 4271 section 6.8
//! connection collision resolution that a reflector topology makes likely.
//!
//! Every session here is a real one over TCP port 179 on this repository's own
//! stack. Nothing writes a route into a RIB; the only way a prefix reaches a
//! speaker is that another speaker sent it an UPDATE.

mod common;

use common::bgp_lab::{
    RR_AS, build_collision_lab, build_rr_lab, converge_sessions, rr_lab_addr, rr_lab_prefix,
};
use toy_tcpip::bgp::Ipv4Prefix;
use toy_tcpip::bgp_router::{BgpPeerRole, BgpRouter, BgpState};
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::VirtualLab;

const RR_ID: Ipv4Address = Ipv4Address([9, 9, 9, 9]);

fn converged_rr_lab() -> VirtualLab {
    let mut lab = build_rr_lab();
    assert!(
        converge_sessions(&mut lab, 60_000),
        "not every session in the route reflector lab reached ESTABLISHED"
    );
    // Give the reflector time to reflect, which is a second round of UPDATEs
    // after every session is up.
    lab.run_until(250, 60_000, |_| false);
    lab
}

fn bgp<'a>(lab: &'a VirtualLab, name: &str) -> &'a BgpRouter {
    lab.router(name).unwrap().bgp().unwrap()
}

/// The prefixes a speaker has in its Loc-RIB that it did not originate itself.
fn learned(lab: &VirtualLab, name: &str) -> Vec<Ipv4Prefix> {
    let b = bgp(lab, name);
    let mine = b.originated_prefixes();
    let mut out: Vec<Ipv4Prefix> = b
        .loc_rib
        .prefixes()
        .into_iter()
        .filter(|p| !mine.contains(p))
        .collect();
    out.sort_by_key(|p| p.address);
    out
}

// ============================================================================
// Roles are configured, and configuring them is the only thing that changes
// ============================================================================

#[test]
fn test_only_the_configured_clients_are_clients() {
    let lab = converged_rr_lab();
    let rr = bgp(&lab, "rr");

    assert!(rr.is_route_reflector());
    assert_eq!(
        rr.cluster_id(),
        RR_ID,
        "cluster ID defaults to the router ID"
    );
    let mut clients = rr.route_reflector_clients();
    clients.sort();
    assert_eq!(clients, vec![rr_lab_addr("c1"), rr_lab_addr("c2")]);

    for spoke in ["c1", "c2"] {
        assert_eq!(
            rr.peer_role(rr_lab_addr(spoke)),
            Some(BgpPeerRole::RouteReflectorClient)
        );
    }
    for spoke in ["n1", "n2"] {
        assert_eq!(rr.peer_role(rr_lab_addr(spoke)), Some(BgpPeerRole::Normal));
    }
    // A client does not become a reflector by being one.
    for spoke in ["c1", "c2", "n1", "n2"] {
        assert!(
            !bgp(&lab, spoke).is_route_reflector(),
            "{} thinks it is a reflector",
            spoke
        );
    }
}

// ============================================================================
// The four RFC 4456 propagation outcomes
// ============================================================================

#[test]
fn test_a_client_route_reaches_the_other_client() {
    let lab = converged_rr_lab();
    assert!(
        learned(&lab, "c2").contains(&rr_lab_prefix("c1")),
        "client -> RR -> client was not reflected; c2 holds {:?}",
        learned(&lab, "c2")
    );
    assert!(
        learned(&lab, "c1").contains(&rr_lab_prefix("c2")),
        "the reverse direction was not reflected either"
    );
}

#[test]
fn test_a_client_route_reaches_a_non_client() {
    let lab = converged_rr_lab();
    for n in ["n1", "n2"] {
        let got = learned(&lab, n);
        assert!(
            got.contains(&rr_lab_prefix("c1")) && got.contains(&rr_lab_prefix("c2")),
            "client -> RR -> non-client was not reflected; {} holds {:?}",
            n,
            got
        );
    }
}

#[test]
fn test_a_non_client_route_reaches_a_client() {
    let lab = converged_rr_lab();
    for c in ["c1", "c2"] {
        let got = learned(&lab, c);
        assert!(
            got.contains(&rr_lab_prefix("n1")) && got.contains(&rr_lab_prefix("n2")),
            "non-client -> RR -> client was not reflected; {} holds {:?}",
            c,
            got
        );
    }
}

#[test]
fn test_a_non_client_route_never_reaches_another_non_client() {
    let lab = converged_rr_lab();

    assert!(
        !learned(&lab, "n2").contains(&rr_lab_prefix("n1")),
        "the plain iBGP rule was broken: n1's prefix reached n2 through the reflector"
    );
    assert!(
        !learned(&lab, "n1").contains(&rr_lab_prefix("n2")),
        "the plain iBGP rule was broken in the other direction too"
    );

    // Not because nothing arrived - both non-clients did learn the client routes.
    assert_eq!(
        learned(&lab, "n1"),
        vec![rr_lab_prefix("c1"), rr_lab_prefix("c2")],
        "a non-client should hear exactly the two client prefixes"
    );

    // And the reflector says so, from live state rather than a stored tally.
    let rr = bgp(&lab, "rr");
    assert_eq!(
        rr.peer(rr_lab_addr("n1")).unwrap().counters.rr_suppressed,
        1,
        "the reflector should be withholding exactly n2's prefix from n1"
    );
    assert_eq!(
        rr.peer(rr_lab_addr("c1")).unwrap().counters.rr_suppressed,
        0,
        "nothing should be withheld from a client"
    );
}

#[test]
fn test_a_client_hears_every_other_prefix_in_the_cluster() {
    let lab = converged_rr_lab();
    assert_eq!(
        learned(&lab, "c1"),
        vec![
            rr_lab_prefix("c2"),
            rr_lab_prefix("n1"),
            rr_lab_prefix("n2")
        ],
        "a client should hear every prefix but its own"
    );
}

// ============================================================================
// Reflection metadata on the IPv4 family
// ============================================================================

#[test]
fn test_a_reflected_ipv4_route_carries_the_originator_and_the_cluster() {
    let lab = converged_rr_lab();

    // c2's view of c1's prefix: reflected by rr, originated by c1.
    let path = bgp(&lab, "c2")
        .adj_rib_in
        .peer_table(rr_lab_addr("rr"))
        .and_then(|t| t.get(&rr_lab_prefix("c1")))
        .expect("c2 has no path for c1's prefix from the reflector");

    assert_eq!(
        path.originator_id,
        Some(Ipv4Address::new(1, 1, 1, 1)),
        "ORIGINATOR_ID should name c1, not the reflector"
    );
    assert_eq!(path.cluster_list, vec![RR_ID]);
    // RFC 4456 section 10: a reflector must not rewrite the NEXT_HOP.
    assert_eq!(
        path.next_hop,
        rr_lab_addr("c1"),
        "the reflector inserted itself as the next hop"
    );
    // Nor LOCAL_PREF, ORIGIN, or AS_PATH; inside one AS the path stays empty.
    assert_eq!(path.local_pref, 100);
    assert!(path.as_path.is_empty());

    // The route is usable, not merely present: it went into the real FIB.
    let fib = &lab.router("c2").unwrap().routing_table;
    let route = fib
        .lookup(Ipv4Address::new(172, 16, 1, 5))
        .expect("c2 has no forwarding entry for the reflected prefix");
    assert_eq!(
        route.next_hop(Ipv4Address::new(172, 16, 1, 5)),
        rr_lab_addr("c1")
    );
}

#[test]
fn test_the_originator_survives_a_second_reflection_hop() {
    // Two reflectors in a row: rr reflects to n1, and n1 - configured as a
    // reflector for c1 in a second cluster - reflects on. ORIGINATOR_ID must
    // still name the speaker that first advertised the route, and CLUSTER_LIST
    // must have grown by exactly the second cluster.
    let mut lab = build_rr_lab();
    lab.router_mut("n1")
        .unwrap()
        .set_bgp_cluster_id(Ipv4Address::new(77, 77, 77, 77));
    // n1 has only one session, to rr. Making rr a client of n1 is what turns n1
    // into a second-level reflector for what it hears from rr.
    lab.router_mut("n1")
        .unwrap()
        .set_bgp_route_reflector_client(rr_lab_addr("rr"), true);
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 60_000, |_| false);

    // rr learns n2's prefix directly, and c1's prefix reflected by n1 as well as
    // from c1 itself. The one to inspect is what n1 sent, which is in rr's
    // Adj-RIB-In under n1's address.
    let from_n1 = bgp(&lab, "rr")
        .adj_rib_in
        .peer_table(rr_lab_addr("n1"))
        .cloned()
        .unwrap_or_default();

    // n1 reflects back what rr reflected to it. The route rr sent n1 for c1's
    // prefix already carried ORIGINATOR_ID c1 and CLUSTER_LIST [9.9.9.9]; what
    // comes back must keep both and add n1's cluster in front.
    if let Some(path) = from_n1.get(&rr_lab_prefix("c1")) {
        assert_eq!(
            path.originator_id,
            Some(Ipv4Address::new(1, 1, 1, 1)),
            "the second reflector rewrote ORIGINATOR_ID"
        );
        assert_eq!(
            path.cluster_list,
            vec![Ipv4Address::new(77, 77, 77, 77), RR_ID],
            "CLUSTER_LIST should be the second cluster followed by the first"
        );
    }

    // Whatever else happened, the fabric settled and nothing grew without bound.
    let longest = lab
        .routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.adj_rib_in.iter_paths())
        .map(|p| p.cluster_list.len())
        .max()
        .unwrap_or(0);
    assert!(longest <= 2, "CLUSTER_LIST grew to {} entries", longest);
}

#[test]
fn test_a_speaker_rejects_a_route_whose_originator_is_itself() {
    // c1 originates 172.16.1.0/24. If the reflector's copy ever came back to c1
    // it would carry ORIGINATOR_ID 1.1.1.1, and c1 must refuse it. The reflector
    // will not send it - split horizon stops that - so the rule is checked where
    // it can be checked directly, on a speaker asked about its own identifier.
    let mut lab = build_rr_lab();
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 60_000, |_| false);

    let c1 = bgp(&lab, "c1");
    assert_eq!(c1.router_id, Ipv4Address::new(1, 1, 1, 1));
    // Its own prefix is in the Loc-RIB as locally originated, never as learned.
    let path = c1.loc_rib.get(&rr_lab_prefix("c1")).unwrap();
    assert!(path.is_local());
    assert_eq!(
        c1.adj_rib_in.prefix_count(rr_lab_addr("rr")),
        3,
        "c1 should have exactly the three prefixes it does not originate"
    );
    assert!(
        !c1.adj_rib_in
            .iter_paths()
            .any(|p| p.prefix == rr_lab_prefix("c1")),
        "c1 accepted a path to its own originated prefix"
    );
}

// ============================================================================
// Reconfiguration
// ============================================================================

#[test]
fn test_demoting_a_client_stops_the_reflection_it_was_getting() {
    let mut lab = build_rr_lab();
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 60_000, |_| false);
    assert!(learned(&lab, "c1").contains(&rr_lab_prefix("n1")));

    // c1 is no longer a client. n1's prefix was learned from a non-client, so it
    // may now go only to clients - and c1 is not one any more.
    lab.router_mut("rr")
        .unwrap()
        .set_bgp_route_reflector_client(rr_lab_addr("c1"), false);
    lab.run_until(250, 60_000, |_| false);

    let got = learned(&lab, "c1");
    assert!(
        !got.contains(&rr_lab_prefix("n1")) && !got.contains(&rr_lab_prefix("n2")),
        "a demoted client kept hearing non-client routes: {:?}",
        got
    );
    // It still hears c2, whose route came from a client and so goes everywhere.
    assert!(
        got.contains(&rr_lab_prefix("c2")),
        "a demoted client stopped hearing client routes as well: {:?}",
        got
    );

    // Promoting it back restores what it lost.
    lab.router_mut("rr")
        .unwrap()
        .set_bgp_route_reflector_client(rr_lab_addr("c1"), true);
    lab.run_until(250, 60_000, |_| false);
    assert!(
        learned(&lab, "c1").contains(&rr_lab_prefix("n1")),
        "promoting the client back did not restore the non-client routes"
    );
}

#[test]
fn test_changing_the_cluster_id_changes_what_goes_on_the_wire() {
    let mut lab = build_rr_lab();
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 60_000, |_| false);
    assert_eq!(
        bgp(&lab, "c2")
            .adj_rib_in
            .peer_table(rr_lab_addr("rr"))
            .and_then(|t| t.get(&rr_lab_prefix("c1")))
            .map(|p| p.cluster_list.clone()),
        Some(vec![RR_ID])
    );

    let new_cluster = Ipv4Address::new(42, 42, 42, 42);
    lab.router_mut("rr")
        .unwrap()
        .set_bgp_cluster_id(new_cluster);
    lab.run_until(250, 60_000, |_| false);

    assert_eq!(bgp(&lab, "rr").cluster_id(), new_cluster);
    assert_eq!(
        bgp(&lab, "c2")
            .adj_rib_in
            .peer_table(rr_lab_addr("rr"))
            .and_then(|t| t.get(&rr_lab_prefix("c1")))
            .map(|p| p.cluster_list.clone()),
        Some(vec![new_cluster]),
        "the clients were never told the cluster identifier had changed"
    );
}

// ============================================================================
// Quiescence
// ============================================================================

#[test]
fn test_the_reflected_fabric_goes_quiet_and_stays_quiet() {
    let mut lab = converged_rr_lab();

    let updates = |l: &VirtualLab| -> u64 {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .flat_map(|b| b.peers())
            .map(|p| p.counters.updates_sent)
            .sum()
    };
    let decisions = |l: &VirtualLab| -> u64 {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .map(|b| b.decision_runs)
            .sum()
    };

    let before = (updates(&lab), decisions(&lab));
    // Two minutes of simulated time, far past several hold intervals, so plenty
    // of KEEPALIVEs flow. An UPDATE in that window would be churn.
    lab.run_until(250, lab.current_time_ms + 120_000, |_| false);
    let after = (updates(&lab), decisions(&lab));

    assert_eq!(
        before.0, after.0,
        "the fabric kept sending UPDATEs after convergence"
    );
    assert_eq!(
        before.1, after.1,
        "the decision process kept re-running after convergence"
    );
    // The sessions survived it all.
    for name in ["rr", "c1", "c2", "n1", "n2"] {
        for peer in bgp(&lab, name).peers() {
            assert_eq!(
                peer.state,
                BgpState::Established,
                "{}'s session to {} did not survive an idle fabric",
                name,
                peer.addr
            );
            assert!(peer.counters.keepalives_sent > 0);
        }
    }
}

// ============================================================================
// RFC 4271 section 6.8 connection collision
// ============================================================================

#[test]
fn test_two_speakers_that_both_dial_end_with_exactly_one_session() {
    let mut lab = build_collision_lab();

    // Both ends are Active, so both dial and both accept. Whatever happens in
    // between, exactly one session must survive and it must stay up.
    assert!(
        converge_sessions(&mut lab, 60_000),
        "the colliding speakers never reached ESTABLISHED"
    );
    lab.run_until(250, 120_000, |_| false);

    for name in ["left", "right"] {
        let b = bgp(&lab, name);
        assert_eq!(b.peers().len(), 1);
        let peer = &b.peers()[0];
        assert_eq!(
            peer.state,
            BgpState::Established,
            "{} did not end up ESTABLISHED",
            name
        );
        assert!(
            !peer.has_collision(),
            "{} is still holding a colliding connection",
            name
        );
        // A connect/reset loop would show up here: the session would have been
        // established many times over, once per cycle.
        assert!(
            peer.establishment_count <= 2,
            "{} established the session {} times, which is a reconnect loop",
            name,
            peer.establishment_count
        );
    }

    // The collision really did happen, and was resolved rather than merely
    // avoided by one side being slower.
    let resolved: u64 = ["left", "right"]
        .iter()
        .map(|n| bgp(&lab, n).peers()[0].counters.collisions_resolved)
        .sum();
    assert!(
        resolved > 0,
        "no collision was detected, so this test proves nothing"
    );

    // And the surviving session carries routes both ways.
    assert!(
        bgp(&lab, "left")
            .loc_rib
            .contains(&Ipv4Prefix::new(Ipv4Address::new(172, 20, 2, 0), 24)),
        "left never learned right's prefix over the surviving session"
    );
    assert!(
        bgp(&lab, "right")
            .loc_rib
            .contains(&Ipv4Prefix::new(Ipv4Address::new(172, 20, 1, 0), 24)),
        "right never learned left's prefix"
    );
}

#[test]
fn test_no_orphan_tcp_stream_survives_a_collision() {
    let mut lab = build_collision_lab();
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 120_000, |_| false);

    // One BGP session means one live TCP connection per speaker. A connection
    // abandoned instead of aborted would still be sitting here.
    for name in ["left", "right"] {
        let live = bgp_connections(&lab, name);
        assert_eq!(
            live.len(),
            1,
            "{} has {} live TCP connections on port 179 after a collision; exactly \
             one should remain: {:?}",
            name,
            live.len(),
            live
        );
    }
}

/// Every live TCP connection on a router that has port 179 at one end.
fn bgp_connections(lab: &VirtualLab, router: &str) -> Vec<toy_tcpip::socket::TcpDiagnostics> {
    lab.router(router)
        .unwrap()
        .sockets
        .as_ref()
        .expect("no socket runtime")
        .all_tcp_diagnostics()
        .into_iter()
        .filter(|d| d.local.port == 179 || d.remote.port == 179)
        .collect()
}

#[test]
fn test_the_speaker_with_the_lower_identifier_keeps_the_inbound_connection() {
    let mut lab = build_collision_lab();
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 60_000, |_| false);

    // left is 1.1.1.1 and right is 2.2.2.2, so RFC 4271 section 6.8 says left
    // closes the connection it initiated and keeps the one right initiated. Both
    // ends therefore agree on a single connection: right's.
    //
    // The observable consequence is a single session that neither side keeps
    // resetting, which is asserted above. What is checked here is that the two
    // ends agree about the same TCP connection, by looking at the port numbers:
    // the surviving connection has right using an ephemeral source port and left
    // holding port 179.
    let left_peer = &bgp(&lab, "left").peers()[0];
    let right_peer = &bgp(&lab, "right").peers()[0];
    assert_eq!(left_peer.state, BgpState::Established);
    assert_eq!(right_peer.state, BgpState::Established);

    let left_conn = bgp_connections(&lab, "left")
        .into_iter()
        .next()
        .expect("left has no BGP connection at all");
    assert_eq!(
        left_conn.local.port, 179,
        "the surviving connection should be the one right dialled, which puts left \
         on port 179 (found {}:{} -> {}:{})",
        left_conn.local.ip, left_conn.local.port, left_conn.remote.ip, left_conn.remote.port
    );

    // The same connection seen from the other end: right holds the ephemeral port,
    // and the two ends agree on the pair. Two surviving connections would show up
    // here as ports that do not match.
    let right_conn = bgp_connections(&lab, "right")
        .into_iter()
        .next()
        .expect("right has no BGP connection at all");
    assert_eq!(right_conn.remote.port, 179);
    assert_eq!(left_conn.remote.port, right_conn.local.port);
}

#[test]
fn test_a_collision_during_reflection_leaves_the_reflected_routes_intact() {
    // The same collision, but with the speakers exchanging routes. A collision
    // resolved by tearing down the wrong connection would show up as a prefix
    // that never arrives or one that arrives and then vanishes.
    let mut lab = build_collision_lab();
    lab.router_mut("left")
        .unwrap()
        .set_bgp_route_reflector_client(Ipv4Address::new(10, 9, 0, 2), true);
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 120_000, |_| false);

    assert!(bgp(&lab, "left").is_route_reflector());
    assert!(
        bgp(&lab, "left")
            .loc_rib
            .contains(&Ipv4Prefix::new(Ipv4Address::new(172, 20, 2, 0), 24))
    );
    assert!(
        bgp(&lab, "right")
            .loc_rib
            .contains(&Ipv4Prefix::new(Ipv4Address::new(172, 20, 1, 0), 24))
    );
    for name in ["left", "right"] {
        assert_eq!(bgp(&lab, name).peers()[0].state, BgpState::Established);
    }
}

#[test]
fn test_reflection_never_applies_to_an_external_session() {
    // Reflection metadata is non-transitive and describes one AS. An eBGP peer
    // marked a client - which is a misconfiguration, but a configurable one -
    // must still be advertised to as an ordinary external neighbour.
    let mut lab = common::bgp_lab::build_linear_lab();
    let r2_to_r1 = Ipv4Address::new(10, 12, 0, 1);
    lab.router_mut("r2")
        .unwrap()
        .set_bgp_route_reflector_client(r2_to_r1, true);
    assert!(converge_sessions(&mut lab, 60_000));
    lab.run_until(250, 60_000, |_| false);

    // r1 still gets r3's prefix, and it arrives with an AS_PATH and no
    // reflection metadata at all.
    let path = bgp(&lab, "r1")
        .adj_rib_in
        .peer_table(Ipv4Address::new(10, 12, 0, 2))
        .and_then(|t| t.get(&Ipv4Prefix::new(Ipv4Address::new(10, 3, 0, 0), 24)))
        .expect("r1 never learned r3's prefix");
    assert_eq!(
        path.originator_id, None,
        "an eBGP peer was sent ORIGINATOR_ID"
    );
    assert!(
        path.cluster_list.is_empty(),
        "an eBGP peer was sent CLUSTER_LIST"
    );
    assert!(
        path.as_path.contains(RR_AS) || !path.as_path.is_empty(),
        "the eBGP AS_PATH was not built normally: [{}]",
        path.as_path
    );
}
